//! 媒体歌曲变化、缓存预展示与解析任务启动的协调。

use std::{path::PathBuf, sync::atomic::Ordering};

use crate::{
    error::Error,
    lyrics::{
        cache::CacheLookup,
        chinese_conversion,
        matcher::MAX_DURATION_DIFFERENCE_MS,
        model::{LyricsChineseVariant, LyricsResolutionMethod, LyricsSnapshot, LyricsStatus},
        players,
        track::TrackDescriptor,
    },
    media::{MediaPlayer, MediaSessionSnapshot},
};

use super::{LyricsService, pipeline::is_cached_snapshot_displayable};

impl LyricsService {
    /// 接收媒体模块的完整快照变化，时间线轻量事件不会触发此入口。
    pub(crate) fn update_media(&self, snapshot: Option<&MediaSessionSnapshot>) {
        self.update_track(snapshot.and_then(TrackDescriptor::from_snapshot), false);
    }

    fn update_track(&self, track: Option<TrackDescriptor>, preserve_ready: bool) {
        let generation = {
            let Ok(mut current) = self.inner.current_track.lock() else {
                return;
            };
            let (player_changed, matching_timeline_changed) = current
                .as_ref()
                .zip(track.as_ref())
                .map_or((false, false), |(current, next)| {
                    if current.key != next.key {
                        return (false, false);
                    }
                    (
                        current.player != next.player,
                        match (current.duration_ms, next.duration_ms) {
                            (Some(current), Some(next)) => {
                                current.abs_diff(next) > MAX_DURATION_DIFFERENCE_MS
                            }
                            (None, None) => false,
                            (None, Some(_)) | (Some(_), None) => true,
                        },
                    )
                });
            if *current == track && !player_changed && !matching_timeline_changed {
                current.clone_from(&track);
                return;
            }
            current.clone_from(&track);
            // 歌曲身份和代数必须在同一临界区更新，避免旧歌曲获得新代数。
            self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1
        };
        self.start_resolution(track, generation, preserve_ready, false);
    }

    /// 对已经登记为当前歌曲的描述启动一次解析，不再反向修改歌曲身份。
    pub(super) fn start_resolution(
        &self,
        track: Option<TrackDescriptor>,
        generation: u64,
        preserve_ready: bool,
        cache_cleared: bool,
    ) {
        if !self.is_current_generation(generation) {
            return;
        }
        self.reset_resolution_trace(generation);
        let Some(track) = track else {
            self.cancel_resolution();
            self.publish_if_current(
                LyricsSnapshot::unavailable(None, "当前没有支持的歌曲"),
                generation,
            );
            return;
        };
        if !self.lyrics_enabled() {
            self.cancel_resolution();
            self.publish_if_current(
                LyricsSnapshot::unavailable(Some(track.key), "歌词显示已关闭"),
                generation,
            );
            return;
        }
        if track.duration_ms.is_none() {
            log::debug!("当前曲目缺少有效时长，等待时间线确认: {}", track.key);
            self.cancel_resolution();
            self.publish_if_current(LyricsSnapshot::loading(track.key), generation);
            return;
        }
        let is_preview =
            players::is_preview_playback(&track, self.cache_path(track.player).as_deref())
                .inspect_err(|error| log::debug!("识别播放器试听状态失败: {error}"))
                .unwrap_or(false);
        if is_preview {
            self.cancel_resolution();
            self.publish_if_current(
                LyricsSnapshot::unavailable(Some(track.key), "试听播放不显示歌词"),
                generation,
            );
            return;
        }
        let keeps_current_content = preserve_ready
            && self.inner.runtime_state.read().is_ok_and(|state| {
                matches!(
                    state.snapshot.status,
                    LyricsStatus::Ready | LyricsStatus::Instrumental | LyricsStatus::NoLyrics
                ) && state.snapshot.track_key.as_ref() == Some(&track.key)
            });
        // 解析期间继续展示已有缓存，同时保留它参与本轮候选比较。
        let mut cached = self.inner.cache.load(&track.key);
        self.prepare_cached_variant(&mut cached, generation);
        let displayable_cache = cached
            .as_ref()
            .filter(|cached| !cached.requires_original_refresh)
            .filter(|cached| is_cached_snapshot_displayable(&track, &cached.snapshot));
        if let Some(cached) = displayable_cache {
            self.publish_if_current_with_method(
                cached.snapshot.clone(),
                generation,
                LyricsResolutionMethod::ApplicationCache,
            );
        } else if !keeps_current_content {
            self.publish_if_current(LyricsSnapshot::loading(track.key.clone()), generation);
        }
        self.enqueue_resolution(track, generation, cached, cache_cleared);
    }

    /// 让缓存字形与当前设置一致；转换只覆盖同一键，且不改变来源刷新时间。
    fn prepare_cached_variant(&self, cached: &mut Option<CacheLookup>, generation: u64) {
        let Some(entry) = cached.as_mut() else {
            return;
        };
        let target = self.preferences().chinese_variant;
        if entry.chinese_variant == target {
            return;
        }
        if target == LyricsChineseVariant::Original {
            entry.requires_original_refresh = true;
            return;
        }

        let previous = entry.chinese_variant;
        let (snapshot, text_converted) =
            chinese_conversion::convert_snapshot_with_outcome(entry.snapshot.clone(), target);
        entry.snapshot = snapshot;
        entry.variant_text_converted = text_converted;
        entry.variant_rewrite_succeeded = match self.inner.cache.rewrite_variant(
            &entry.snapshot,
            target,
            generation,
            entry.refreshed_at_seconds,
        ) {
            Ok(()) => true,
            Err(error) => {
                log::warn!("更新缓存歌词字形失败: {error}");
                false
            }
        };
        entry.chinese_variant = target;
        entry.variant_updated_from = Some(previous);
    }

    pub(super) fn cache_path(&self, player: MediaPlayer) -> Option<PathBuf> {
        self.inner
            .adapter_paths
            .read()
            .ok()
            .and_then(|paths| paths.get(&player).cloned().flatten())
    }

    pub(super) fn prepare_player_resolution(
        &self,
        player: MediaPlayer,
    ) -> Result<Option<(TrackDescriptor, u64)>, Error> {
        let current = self
            .inner
            .current_track
            .lock()
            .map_err(|_| Error::Message("当前歌曲状态不可用".to_owned()))?;
        let Some(track) = current.as_ref().filter(|track| track.player == player) else {
            return Ok(None);
        };
        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        Ok(Some((track.clone(), generation)))
    }

    pub(super) fn prepare_track_resolution(
        &self,
        player: MediaPlayer,
        expected_track_key: &str,
    ) -> Result<Option<(TrackDescriptor, u64)>, Error> {
        let current = self
            .inner
            .current_track
            .lock()
            .map_err(|_| Error::Message("当前歌曲状态不可用".to_owned()))?;
        let Some(track) = current
            .as_ref()
            .filter(|track| track.player == player && track.key == expected_track_key)
        else {
            return Ok(None);
        };
        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        Ok(Some((track.clone(), generation)))
    }

    pub(super) fn start_prepared_resolution(
        &self,
        pending: Option<(TrackDescriptor, u64)>,
        preserve_ready: bool,
        cache_cleared: bool,
    ) {
        if let Some((track, generation)) = pending {
            self.start_resolution(Some(track), generation, preserve_ready, cache_cleared);
        }
    }
}
