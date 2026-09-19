use std::sync::atomic::Ordering;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::lyrics::{
    model::{LyricsSourceKind, ResolvedLyrics, has_word_timing},
    watcher,
};

use super::{LyricsService, MediaPlayer, is_plausible_timeline, lookup_hit, players};

impl LyricsService {
    /// 按播放器分别重建非递归监听器，避免其他播放器的写入刷新当前歌词。
    pub(super) fn refresh_watchers(&self) {
        if self.inner.shutdown_requested.load(Ordering::Acquire) {
            return;
        }
        let next_paths = players::supported_players()
            .map(|player| (player, players::automatic_cache_path(player)))
            .collect::<HashMap<_, _>>();
        if let Ok(mut current) = self.inner.adapter_paths.write() {
            current.clone_from(&next_paths);
        }
        let mut next_watchers = HashMap::new();
        for player in players::supported_players() {
            let cache_path = next_paths.get(&player).cloned().flatten();
            let paths = players::watch_paths_for(player, cache_path.as_deref());
            let weak_inner = Arc::downgrade(&self.inner);
            let callback: Arc<dyn Fn(Vec<PathBuf>) + Send + Sync> = Arc::new(move |paths| {
                let Some(inner) = weak_inner.upgrade() else {
                    return;
                };
                if inner.shutdown_requested.load(Ordering::Acquire) {
                    return;
                }
                let service = LyricsService { inner };
                // 歌词关闭时缓存写入不产生任何工作，避免仍重建索引或读取播放器本地文件。
                if !service.lyrics_enabled() {
                    return;
                }
                let configuration_changed = players::configuration_changed(player, &paths);
                let result = if configuration_changed {
                    service.handle_configuration_change(player)
                } else {
                    service.handle_cache_content_change(player, &paths)
                };
                if let Err(error) = result {
                    log::warn!("响应 {player:?} 歌词缓存变化失败: {error}");
                }
            });
            match watcher::create(paths, callback, Arc::clone(&self.inner.watcher_metrics)) {
                Ok(Some(watcher)) => {
                    next_watchers.insert(player, watcher);
                }
                Ok(None) => {}
                Err(error) => log::warn!("建立 {player:?} 歌词缓存监听失败: {error}"),
            }
        }
        let previous = self.inner.watchers.lock().ok().map(|mut current| {
            if self.inner.shutdown_requested.load(Ordering::Acquire) {
                return next_watchers;
            }
            std::mem::replace(&mut *current, next_watchers)
        });
        // watcher Drop 会 join；必须在释放 service mutex 后执行。
        drop(previous);
    }

    /// 启动适配器声明的原生缓存路径设置监听；回调只持有服务的弱引用。
    pub(super) fn start_registry_watcher(&self) {
        let weak_inner = Arc::downgrade(&self.inner);
        let next_watchers = players::watch_registry_settings(Arc::new(move |player| {
            let Some(inner) = weak_inner.upgrade() else {
                return;
            };
            if inner.shutdown_requested.load(Ordering::Acquire) {
                return;
            }
            let service = LyricsService { inner };
            if let Err(error) = service.handle_configuration_change(player) {
                log::warn!("响应 {player:?} 缓存目录设置变化失败: {error}");
            }
        }));
        let previous = self.inner.registry_watchers.lock().ok().map(|mut current| {
            if self.inner.shutdown_requested.load(Ordering::Acquire) {
                return next_watchers;
            }
            std::mem::replace(&mut *current, next_watchers)
        });
        // watcher Drop 会 join；必须在释放 service mutex 后执行。
        drop(previous);
    }

    /// 歌词关闭时释放全部文件与注册表监听，避免后台线程继续响应缓存写入。
    pub(super) fn release_watchers(&self) {
        let previous_watchers = self
            .inner
            .watchers
            .lock()
            .ok()
            .map(|mut current| std::mem::take(&mut *current));
        let previous_registry = self
            .inner
            .registry_watchers
            .lock()
            .ok()
            .map(|mut current| std::mem::take(&mut *current));
        // watcher Drop 会 join；必须在释放 service mutex 后执行。
        drop(previous_watchers);
        drop(previous_registry);
    }

    /// 播放器目录配置变化属于低频事件，需要重建监听并淘汰旧来源结果。
    fn handle_configuration_change(&self, player: MediaPlayer) -> Result<(), String> {
        if !self.lyrics_enabled() {
            return Ok(());
        }
        let pending = self.prepare_player_resolution(player)?;
        self.refresh_watchers();
        if let Err(error) = self.inner.cache.clear_local_source(player) {
            log::warn!("清理播放器旧本地歌词缓存失败: {error}");
        }
        self.start_prepared_resolution(pending, true);
        Ok(())
    }

    /// 只淘汰当前播放器、当前歌曲的结果，其他播放器写缓存时不做任何工作。
    fn handle_cache_content_change(
        &self,
        player: MediaPlayer,
        paths: &[PathBuf],
    ) -> Result<(), String> {
        if !self.lyrics_enabled() {
            return Ok(());
        }
        let cache_path = self.cache_path(player);
        // notify 事件可能早于 Windows 目录修改时间更新，先按事件事实淘汰旧索引。
        players::invalidate_local_index(player, cache_path.as_deref());
        let watched_paths = players::watch_paths_for(player, cache_path.as_deref());
        let watch_root_changed = paths.iter().any(|changed| {
            watched_paths
                .iter()
                .any(|watched| watcher::paths_equivalent(changed, watched))
        });
        if watch_root_changed {
            self.refresh_watchers();
        }
        let track = self
            .inner
            .current_track
            .lock()
            .map_err(|_| "当前歌曲状态不可用".to_owned())?
            .as_ref()
            .filter(|track| track.player == player)
            .cloned();
        let Some(track) = track else {
            return Ok(());
        };
        let (current_source, current_has_word_timing) = self
            .inner
            .runtime_state
            .read()
            .ok()
            .filter(|state| state.snapshot.track_key.as_ref() == Some(&track.key))
            .map_or((None, false), |state| {
                (
                    state.snapshot.source.clone(),
                    has_word_timing(&state.snapshot.lines),
                )
            });
        if !watch_root_changed
            && !players::changed_paths_affect_track(
                &track,
                cache_path.as_deref(),
                paths,
                current_source.as_ref(),
                current_has_word_timing,
            )
        {
            return Ok(());
        }
        let current_uses_player_local = current_source.as_ref().is_some_and(|source| {
            source.player == player && source.kind == LyricsSourceKind::Local
        });
        let should_compare_local = current_uses_player_local || current_has_word_timing;
        let local_after_change = should_compare_local
            .then(|| players::resolve_current_local(&track, cache_path.clone()));
        if let Some(Err(error)) = local_after_change.as_ref() {
            log::debug!("检查播放器本地歌词变化失败: {error}");
        }
        if current_uses_player_local
            && local_after_change
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .and_then(lookup_hit)
                .is_some_and(|local| self.current_snapshot_matches(&track.key, local))
        {
            return Ok(());
        }
        // 播放器切歌会改写队列和行级歌词缓存，不能因此淘汰其他来源的逐字结果。
        if current_has_word_timing && !current_uses_player_local {
            let local_can_replace_word_timing = local_after_change
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .and_then(lookup_hit)
                .is_some_and(|local| {
                    is_plausible_timeline(&track, &local.lines) && has_word_timing(&local.lines)
                });
            if !local_can_replace_word_timing {
                return Ok(());
            }
        }
        let pending = self.prepare_track_resolution(player, &track.key)?;
        let Some((track, generation)) = pending else {
            return Ok(());
        };
        if let Err(error) = self.inner.cache.remove(&track.key) {
            log::warn!("清理当前歌曲解析缓存失败: {error}");
        }
        self.start_resolution(Some(track), generation, true);
        Ok(())
    }

    /// 比较播放器事件后的本地结果与当前快照，忽略仅触碰文件但内容未变的事件。
    fn current_snapshot_matches(&self, track_key: &str, local: &ResolvedLyrics) -> bool {
        self.inner.runtime_state.read().is_ok_and(|state| {
            state.snapshot.track_key.as_deref() == Some(track_key)
                && state.snapshot.source.as_ref() == Some(&local.source)
                && state.snapshot.lines == local.lines
        })
    }
}
