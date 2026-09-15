use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use reqwest::blocking::Client;

use crate::media::MediaPlayer;

use super::super::{
    error::LyricsError,
    model::{
        LyricsLookupMiss, LyricsLookupOutcome, LyricsSource, LyricsSourceKind, ResolvedLyrics,
    },
    network::ResolutionDeadline,
    track::TrackDescriptor,
};
use super::registry_watch::RegistryWatchHandle;
use super::{kugou_music, netease_cloud_music, qq_music, soda_music};

type LookupResult = Result<LyricsLookupOutcome, LyricsError>;
type LocalResolver = fn(&TrackDescriptor, Option<&Path>) -> LookupResult;
type OnlineResolver =
    fn(&TrackDescriptor, Option<&Path>, &Client, &ResolutionDeadline) -> LookupResult;
type PreviewDetector = fn(&TrackDescriptor, Option<&Path>) -> Result<bool, LyricsError>;
type CachePathResolver = fn() -> Option<PathBuf>;
type AdditionalWatchPaths = fn() -> Vec<PathBuf>;
type LocalIndexInvalidator = fn(&Path);
type CacheChangeMatcher =
    fn(&TrackDescriptor, Option<&Path>, &[PathBuf], Option<&LyricsSource>, bool) -> bool;
type RegistryChangeCallback = Arc<dyn Fn() + Send + Sync>;
type RegistryWatcher = fn(RegistryChangeCallback) -> Result<RegistryWatchHandle, io::Error>;

/// 本地歌词读取能力。
#[derive(Clone, Copy)]
struct LocalLyricsCapability {
    resolve: LocalResolver,
}

/// 在线歌词读取能力。
#[derive(Clone, Copy)]
struct OnlineLyricsCapability {
    resolve: OnlineResolver,
}

/// 播放器缓存发现、监听和索引维护能力。
#[derive(Clone, Copy)]
struct LyricsStorageCapability {
    discover_cache_path: CachePathResolver,
    additional_watch_paths: AdditionalWatchPaths,
    configuration_file_name: Option<&'static str>,
    invalidate_local_index: Option<LocalIndexInvalidator>,
    changed_paths_affect_track: CacheChangeMatcher,
    watch_registry_settings: Option<RegistryWatcher>,
}

/// 单个平台的编译期歌词能力描述；能力缺失由对应分组的 `None` 明确表达。
#[derive(Clone, Copy)]
pub(super) struct LyricsAdapter {
    pub player: MediaPlayer,
    local: Option<LocalLyricsCapability>,
    online: Option<OnlineLyricsCapability>,
    storage: LyricsStorageCapability,
    preview: Option<PreviewDetector>,
}

impl LyricsAdapter {
    /// 执行本地能力，不支持时返回统一的 Unsupported。
    pub fn resolve_local(
        &self,
        track: &TrackDescriptor,
        cache_path: Option<&Path>,
    ) -> LookupResult {
        self.local
            .map_or(Ok(LyricsLookupOutcome::Unsupported), |capability| {
                (capability.resolve)(track, cache_path)
            })
    }

    /// 执行在线能力，不支持时返回统一的 Unsupported。
    pub fn resolve_online(
        &self,
        track: &TrackDescriptor,
        cache_path: Option<&Path>,
        client: &Client,
        deadline: &ResolutionDeadline,
    ) -> LookupResult {
        self.online
            .map_or(Ok(LyricsLookupOutcome::Unsupported), |capability| {
                (capability.resolve)(track, cache_path, client, deadline)
            })
    }

    /// 使用播放器元数据识别试听；无专用能力时确定返回 false。
    pub fn is_preview_playback(
        &self,
        track: &TrackDescriptor,
        cache_path: Option<&Path>,
    ) -> Result<bool, LyricsError> {
        self.preview
            .map_or(Ok(false), |detect| detect(track, cache_path))
    }

    /// 发现播放器主缓存目录。
    pub fn automatic_cache_path(&self) -> Option<PathBuf> {
        (self.storage.discover_cache_path)()
    }

    /// 合并主缓存和平台额外监听路径。
    pub fn watch_paths(&self, cache_path: Option<&Path>) -> Vec<PathBuf> {
        cache_path
            .map(Path::to_owned)
            .into_iter()
            .chain((self.storage.additional_watch_paths)())
            .collect()
    }

    /// 判断文件事件是否来自平台缓存目录配置文件。
    pub fn configuration_changed(&self, paths: &[PathBuf]) -> bool {
        self.storage
            .configuration_file_name
            .is_some_and(|expected| {
                paths.iter().any(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.eq_ignore_ascii_case(expected))
                })
            })
    }

    /// 淘汰依赖文件名扫描的本地索引。
    pub fn invalidate_local_index(&self, cache_path: Option<&Path>) {
        if let Some((invalidate, cache_path)) = self.storage.invalidate_local_index.zip(cache_path)
        {
            invalidate(cache_path);
        }
    }

    /// 判断缓存文件事件是否可能改变当前歌曲歌词。
    pub fn changed_paths_affect_track(
        &self,
        track: &TrackDescriptor,
        cache_path: Option<&Path>,
        paths: &[PathBuf],
        current_source: Option<&LyricsSource>,
        current_has_word_timing: bool,
    ) -> bool {
        (self.storage.changed_paths_affect_track)(
            track,
            cache_path,
            paths,
            current_source,
            current_has_word_timing,
        )
    }

    /// 返回平台原生缓存路径设置监听能力。
    pub fn registry_watcher(&self) -> Option<RegistryWatcher> {
        self.storage.watch_registry_settings
    }
}

/// 四个播放器只在此处登记一次，新增平台能力时不再同步维护多组分发表。
pub(super) const ADAPTERS: [LyricsAdapter; 4] = [
    LyricsAdapter {
        player: MediaPlayer::QqMusic,
        local: Some(LocalLyricsCapability {
            resolve: resolve_qq_local,
        }),
        online: Some(OnlineLyricsCapability {
            resolve: resolve_qq_online,
        }),
        storage: LyricsStorageCapability {
            discover_cache_path: qq_music::automatic_cache_path,
            additional_watch_paths: no_additional_watch_paths,
            configuration_file_name: None,
            invalidate_local_index: Some(qq_music::invalidate_local_index),
            changed_paths_affect_track: qq_changed_paths_affect_track,
            watch_registry_settings: Some(qq_music::watch_cache_path_changes),
        },
        preview: None,
    },
    LyricsAdapter {
        player: MediaPlayer::NeteaseCloudMusic,
        local: Some(LocalLyricsCapability {
            resolve: resolve_netease_local,
        }),
        online: Some(OnlineLyricsCapability {
            resolve: resolve_netease_online,
        }),
        storage: LyricsStorageCapability {
            discover_cache_path: netease_cloud_music::automatic_cache_path,
            additional_watch_paths: netease_additional_watch_paths,
            configuration_file_name: None,
            invalidate_local_index: None,
            changed_paths_affect_track: netease_changed_paths_affect_track,
            watch_registry_settings: None,
        },
        preview: None,
    },
    LyricsAdapter {
        player: MediaPlayer::SodaMusic,
        local: None,
        online: Some(OnlineLyricsCapability {
            resolve: resolve_soda_online,
        }),
        storage: LyricsStorageCapability {
            discover_cache_path: soda_music::automatic_cache_path,
            additional_watch_paths: no_additional_watch_paths,
            configuration_file_name: None,
            invalidate_local_index: None,
            changed_paths_affect_track: soda_changed_paths_affect_track,
            watch_registry_settings: None,
        },
        preview: Some(soda_music::is_preview_playback),
    },
    LyricsAdapter {
        player: MediaPlayer::KugouMusic,
        local: Some(LocalLyricsCapability {
            resolve: resolve_kugou_local,
        }),
        online: None,
        storage: LyricsStorageCapability {
            discover_cache_path: kugou_music::automatic_cache_path,
            additional_watch_paths: kugou_additional_watch_paths,
            configuration_file_name: Some("KuGou.ini"),
            invalidate_local_index: Some(kugou_music::invalidate_local_index),
            changed_paths_affect_track: kugou_changed_paths_affect_track,
            watch_registry_settings: None,
        },
        preview: None,
    },
];

/// 按公开播放器枚举查找已登记的歌词适配器。
pub(super) fn adapter(player: MediaPlayer) -> Option<&'static LyricsAdapter> {
    ADAPTERS.iter().find(|adapter| adapter.player == player)
}

/// 把平台内部 Option 收敛为 A2 统一结果契约。
fn completed_lookup(resolved: Option<ResolvedLyrics>) -> LyricsLookupOutcome {
    resolved.map_or(
        LyricsLookupOutcome::Miss(LyricsLookupMiss::NoReliableLyrics),
        LyricsLookupOutcome::Hit,
    )
}

fn resolve_qq_local(track: &TrackDescriptor, cache_path: Option<&Path>) -> LookupResult {
    let Some(cache_path) = cache_path else {
        return Ok(LyricsLookupOutcome::Miss(LyricsLookupMiss::DataUnavailable));
    };
    qq_music::resolve_local(track, cache_path).map(completed_lookup)
}

fn resolve_qq_online(
    track: &TrackDescriptor,
    _cache_path: Option<&Path>,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> LookupResult {
    qq_music::resolve_online(track, client, deadline).map(completed_lookup)
}

fn resolve_netease_local(track: &TrackDescriptor, cache_path: Option<&Path>) -> LookupResult {
    let Some(cache_path) = cache_path else {
        return Ok(LyricsLookupOutcome::Miss(LyricsLookupMiss::DataUnavailable));
    };
    netease_cloud_music::resolve_local(track, cache_path).map(completed_lookup)
}

fn resolve_netease_online(
    track: &TrackDescriptor,
    _cache_path: Option<&Path>,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> LookupResult {
    netease_cloud_music::resolve_online(track, client, deadline).map(completed_lookup)
}

fn resolve_soda_online(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> LookupResult {
    let Some(cache_path) = cache_path else {
        return Ok(LyricsLookupOutcome::Miss(LyricsLookupMiss::DataUnavailable));
    };
    soda_music::resolve(track, Some(cache_path), client, deadline).map(completed_lookup)
}

fn resolve_kugou_local(track: &TrackDescriptor, cache_path: Option<&Path>) -> LookupResult {
    let Some(cache_path) = cache_path else {
        return Ok(LyricsLookupOutcome::Miss(LyricsLookupMiss::DataUnavailable));
    };
    kugou_music::resolve(track, Some(cache_path)).map(completed_lookup)
}

fn no_additional_watch_paths() -> Vec<PathBuf> {
    Vec::new()
}

fn netease_additional_watch_paths() -> Vec<PathBuf> {
    netease_cloud_music::additional_watch_path()
        .into_iter()
        .collect()
}

fn kugou_additional_watch_paths() -> Vec<PathBuf> {
    kugou_music::configuration_watch_path()
        .into_iter()
        .collect()
}

fn qq_changed_paths_affect_track(
    track: &TrackDescriptor,
    _cache_path: Option<&Path>,
    paths: &[PathBuf],
    _current_source: Option<&LyricsSource>,
    _current_has_word_timing: bool,
) -> bool {
    qq_music::changed_paths_affect_track(track, paths)
}

fn netease_changed_paths_affect_track(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
    paths: &[PathBuf],
    current_source: Option<&LyricsSource>,
    current_has_word_timing: bool,
) -> bool {
    cache_path.is_some_and(|cache_path| {
        netease_cloud_music::changed_paths_affect_track(
            track,
            cache_path,
            paths,
            current_source,
            current_has_word_timing,
        )
    })
}

fn soda_changed_paths_affect_track(
    _track: &TrackDescriptor,
    _cache_path: Option<&Path>,
    paths: &[PathBuf],
    current_source: Option<&LyricsSource>,
    _current_has_word_timing: bool,
) -> bool {
    let already_uses_soda = current_source.is_some_and(|source| {
        source.player == MediaPlayer::SodaMusic
            && source.kind == LyricsSourceKind::Online
            && source.song_id.is_some()
    });
    !already_uses_soda
        && paths.iter().any(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("QueueCache"))
        })
}

fn kugou_changed_paths_affect_track(
    track: &TrackDescriptor,
    _cache_path: Option<&Path>,
    paths: &[PathBuf],
    _current_source: Option<&LyricsSource>,
    _current_has_word_timing: bool,
) -> bool {
    kugou_music::changed_paths_affect_track(track, paths)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::media::MediaPlayer;

    use super::{ADAPTERS, adapter};

    #[test]
    fn registry_contains_each_player_once() {
        let players = ADAPTERS
            .iter()
            .map(|adapter| adapter.player)
            .collect::<HashSet<_>>();

        assert_eq!(players.len(), ADAPTERS.len());
    }

    #[test]
    fn unsupported_player_has_no_adapter() {
        assert!(adapter(MediaPlayer::Other).is_none());
    }
}
