mod file_index;
mod kugou_music;
mod netease_cloud_music;
mod qq_music;
mod registry;
mod registry_watch;
mod soda_music;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use reqwest::blocking::Client;

use crate::media::MediaPlayer;

use super::{
    error::LyricsError,
    model::{LyricsLookupOutcome, LyricsSource},
    network::ResolutionDeadline,
    track::TrackDescriptor,
};
use registry::{ADAPTERS, LyricsAdapter, adapter};
pub(super) use registry_watch::RegistryWatchHandle;

type RegistryChangeCallback = Arc<dyn Fn(MediaPlayer) + Send + Sync>;

/// 按静态注册顺序返回全部已接入播放器。
pub fn supported_players() -> impl Iterator<Item = MediaPlayer> {
    ADAPTERS.iter().map(|adapter| adapter.player)
}

/// 运行指定平台的在线能力，供 pipeline 组合跨平台兜底。
pub fn resolve_online_for(
    player: MediaPlayer,
    track: &TrackDescriptor,
    cache_path: Option<PathBuf>,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<LyricsLookupOutcome, LyricsError> {
    adapter(player).map_or(Ok(LyricsLookupOutcome::Unsupported), |adapter| {
        adapter.resolve_online(track, cache_path.as_deref(), client, deadline)
    })
}

/// 使用当前播放器的专用元数据识别试听播放。
pub fn is_preview_playback(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
) -> Result<bool, LyricsError> {
    adapter(track.player).map_or(Ok(false), |adapter| {
        adapter.is_preview_playback(track, cache_path)
    })
}

/// 运行当前播放器的本地歌词能力。
pub fn resolve_current_local(
    track: &TrackDescriptor,
    cache_path: Option<PathBuf>,
) -> Result<LyricsLookupOutcome, LyricsError> {
    resolve_local_for(track.player, track, cache_path)
}

/// 运行指定平台的本地歌词能力，供 pipeline 组合跨平台兜底。
pub fn resolve_local_for(
    player: MediaPlayer,
    track: &TrackDescriptor,
    cache_path: Option<PathBuf>,
) -> Result<LyricsLookupOutcome, LyricsError> {
    adapter(player).map_or(Ok(LyricsLookupOutcome::Unsupported), |adapter| {
        adapter.resolve_local(track, cache_path.as_deref())
    })
}

/// 判断播放器缓存事件是否可能改变当前歌曲歌词。
pub fn changed_paths_affect_track(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
    paths: &[PathBuf],
    current_source: Option<&LyricsSource>,
    current_has_word_timing: bool,
) -> bool {
    adapter(track.player).is_some_and(|adapter| {
        adapter.changed_paths_affect_track(
            track,
            cache_path,
            paths,
            current_source,
            current_has_word_timing,
        )
    })
}

/// 淘汰播放器依赖文件名扫描的本地索引。
pub fn invalidate_local_index(player: MediaPlayer, cache_path: Option<&Path>) {
    if let Some(adapter) = adapter(player) {
        adapter.invalidate_local_index(cache_path);
    }
}

/// 发现播放器主缓存目录。
pub fn automatic_cache_path(player: MediaPlayer) -> Option<PathBuf> {
    adapter(player).and_then(LyricsAdapter::automatic_cache_path)
}

/// 使用已发现的主缓存目录构建完整监听路径。
pub fn watch_paths_for(player: MediaPlayer, cache_path: Option<&Path>) -> Vec<PathBuf> {
    adapter(player).map_or_else(Vec::new, |adapter| adapter.watch_paths(cache_path))
}

/// 判断文件事件是否来自播放器缓存目录配置文件。
pub fn configuration_changed(player: MediaPlayer, paths: &[PathBuf]) -> bool {
    adapter(player).is_some_and(|adapter| adapter.configuration_changed(paths))
}

/// 启动所有适配器声明的原生缓存路径设置监听。
pub(super) fn watch_registry_settings(
    on_change: RegistryChangeCallback,
) -> Vec<RegistryWatchHandle> {
    let mut watchers = Vec::new();
    for adapter in ADAPTERS {
        let Some(watch) = adapter.registry_watcher() else {
            continue;
        };
        let on_change = Arc::clone(&on_change);
        match watch(Arc::new(move || on_change(adapter.player))) {
            Ok(watcher) => watchers.push(watcher),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                log::debug!("{:?} 注册表配置不存在，跳过键值监听", adapter.player);
            }
            Err(error) => {
                log::warn!("监听 {:?} 缓存目录设置失败: {error}", adapter.player);
            }
        }
    }
    watchers
}

#[cfg(test)]
mod tests {
    use crate::media::MediaPlayer;

    use super::{LyricsLookupOutcome, TrackDescriptor, resolve_current_local, supported_players};

    /// 构造指定播放器的最小歌曲描述。
    fn track(player: MediaPlayer) -> TrackDescriptor {
        TrackDescriptor {
            key: "track".to_owned(),
            player,
            title: "歌曲".to_owned(),
            artists: vec!["歌手".to_owned()],
            duration_ms: Some(180_000),
        }
    }

    #[test]
    fn supported_players_come_from_registry() {
        assert_eq!(supported_players().count(), 4);
    }

    #[test]
    fn unsupported_local_capability_is_explicit() {
        let outcome = resolve_current_local(&track(MediaPlayer::SodaMusic), None);

        assert!(matches!(outcome, Ok(LyricsLookupOutcome::Unsupported)));
    }
}
