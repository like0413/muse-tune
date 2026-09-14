mod file_index;
mod kugou_music;
mod netease_cloud_music;
mod qq_music;
mod soda_music;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use reqwest::blocking::Client;

use crate::media::MediaPlayer;

use super::{
    error::LyricsError,
    model::{
        LyricsLookupMiss, LyricsLookupOutcome, LyricsSource, LyricsSourceKind, ResolvedLyrics,
    },
    network::ResolutionDeadline,
    track::TrackDescriptor,
};

/// 当前支持的四个播放器，供自动缓存监听复用。
pub const SUPPORTED_PLAYERS: [MediaPlayer; 4] = [
    MediaPlayer::QqMusic,
    MediaPlayer::NeteaseCloudMusic,
    MediaPlayer::SodaMusic,
    MediaPlayer::KugouMusic,
];

/// 只运行当前播放器需要联网的解析入口，本地歌词由协调器提前分层处理。
pub fn resolve_current_online(
    track: &TrackDescriptor,
    cache_path: Option<PathBuf>,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<LyricsLookupOutcome, LyricsError> {
    let result = match track.player {
        MediaPlayer::QqMusic => qq_music::resolve_online(track, client, deadline),
        MediaPlayer::NeteaseCloudMusic => {
            netease_cloud_music::resolve_online(track, client, deadline)
        }
        MediaPlayer::SodaMusic => {
            let Some(cache_path) = cache_path.as_deref() else {
                return Ok(LyricsLookupOutcome::Miss(LyricsLookupMiss::DataUnavailable));
            };
            soda_music::resolve(track, Some(cache_path), client, deadline)
        }
        MediaPlayer::KugouMusic | MediaPlayer::Other => {
            return Ok(LyricsLookupOutcome::Unsupported);
        }
    }?;
    Ok(completed_lookup(result))
}

/// 使用播放器本地元数据识别当前是否为试听播放。
pub fn is_preview_playback(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
) -> Result<bool, LyricsError> {
    if track.player != MediaPlayer::SodaMusic {
        return Ok(false);
    }
    soda_music::is_preview_playback(track, cache_path)
}

/// 只读取当前播放器的本地歌词，用于新鲜持久缓存的低成本升级检查。
pub fn resolve_current_local(
    track: &TrackDescriptor,
    cache_path: Option<PathBuf>,
) -> Result<LyricsLookupOutcome, LyricsError> {
    let Some(cache_path) = cache_path else {
        return Ok(
            if matches!(
                track.player,
                MediaPlayer::QqMusic | MediaPlayer::NeteaseCloudMusic | MediaPlayer::KugouMusic
            ) {
                LyricsLookupOutcome::Miss(LyricsLookupMiss::DataUnavailable)
            } else {
                LyricsLookupOutcome::Unsupported
            },
        );
    };
    let result = match track.player {
        MediaPlayer::QqMusic => qq_music::resolve_local(track, &cache_path),
        MediaPlayer::NeteaseCloudMusic => netease_cloud_music::resolve_local(track, &cache_path),
        MediaPlayer::KugouMusic => kugou_music::resolve(track, Some(&cache_path)),
        MediaPlayer::SodaMusic | MediaPlayer::Other => {
            return Ok(LyricsLookupOutcome::Unsupported);
        }
    }?;
    Ok(completed_lookup(result))
}

/// 判断播放器缓存事件是否确实可能改变当前歌曲的歌词。
pub fn changed_paths_affect_track(
    track: &TrackDescriptor,
    cache_path: Option<&std::path::Path>,
    paths: &[PathBuf],
    current_source: Option<&LyricsSource>,
    current_has_word_timing: bool,
) -> bool {
    match track.player {
        MediaPlayer::QqMusic => qq_music::changed_paths_affect_track(track, paths),
        MediaPlayer::NeteaseCloudMusic => cache_path.is_some_and(|cache_path| {
            netease_cloud_music::changed_paths_affect_track(
                track,
                cache_path,
                paths,
                current_source,
                current_has_word_timing,
            )
        }),
        MediaPlayer::KugouMusic => kugou_music::changed_paths_affect_track(track, paths),
        MediaPlayer::SodaMusic => {
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
        MediaPlayer::Other => false,
    }
}

/// 根据原生文件事件使播放器的文件名索引失效，避免依赖目录时间戳的更新顺序。
pub fn invalidate_local_index(player: MediaPlayer, cache_path: Option<&Path>) {
    let Some(cache_path) = cache_path else {
        return;
    };
    match player {
        MediaPlayer::QqMusic => qq_music::invalidate_local_index(cache_path),
        MediaPlayer::KugouMusic => kugou_music::invalidate_local_index(cache_path),
        MediaPlayer::NeteaseCloudMusic | MediaPlayer::SodaMusic | MediaPlayer::Other => {}
    }
}

/// 使用 QQ 官方域名下的网页内部 HTTPS 接口作为跨播放器兜底。
pub fn resolve_qq_online(
    track: &TrackDescriptor,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<LyricsLookupOutcome, LyricsError> {
    qq_music::resolve_online(track, client, deadline).map(completed_lookup)
}

/// 仅查询 QQ 音乐本地 QRC，供协调器为其他播放器补足真实逐字时间轴。
pub fn resolve_qq_local(
    track: &TrackDescriptor,
    cache_path: Option<PathBuf>,
) -> Result<LyricsLookupOutcome, LyricsError> {
    let Some(cache_path) = cache_path else {
        return Ok(LyricsLookupOutcome::Miss(LyricsLookupMiss::DataUnavailable));
    };
    qq_music::resolve_local(track, &cache_path).map(completed_lookup)
}

/// 使用网易云官方域名下的网页内部 HTTPS 接口作为跨播放器兜底。
pub fn resolve_netease_online(
    track: &TrackDescriptor,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<LyricsLookupOutcome, LyricsError> {
    netease_cloud_music::resolve_online(track, client, deadline).map(completed_lookup)
}

/// 把平台内部查找结果收敛为协调器可替换的统一业务契约。
fn completed_lookup(resolved: Option<ResolvedLyrics>) -> LyricsLookupOutcome {
    resolved.map_or(
        LyricsLookupOutcome::Miss(LyricsLookupMiss::NoReliableLyrics),
        LyricsLookupOutcome::Hit,
    )
}

/// 将路径自动发现委派给对应播放器模块。
pub fn automatic_cache_path(player: MediaPlayer) -> Option<PathBuf> {
    match player {
        MediaPlayer::QqMusic => qq_music::automatic_cache_path(),
        MediaPlayer::NeteaseCloudMusic => netease_cloud_music::automatic_cache_path(),
        MediaPlayer::SodaMusic => soda_music::automatic_cache_path(),
        MediaPlayer::KugouMusic => kugou_music::automatic_cache_path(),
        MediaPlayer::Other => None,
    }
}

/// 返回单个平台除主缓存外还需观察的数据或配置目录。
fn additional_watch_paths(player: MediaPlayer) -> Vec<PathBuf> {
    match player {
        MediaPlayer::NeteaseCloudMusic => netease_cloud_music::additional_watch_path()
            .into_iter()
            .collect(),
        MediaPlayer::KugouMusic => kugou_music::configuration_watch_path()
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

/// 使用已经发现的主缓存目录构建监听路径，避免重复读取注册表或配置文件。
pub fn watch_paths_for(player: MediaPlayer, cache_path: Option<&Path>) -> Vec<PathBuf> {
    cache_path
        .map(Path::to_owned)
        .into_iter()
        .chain(additional_watch_paths(player))
        .collect()
}

/// QQ 的缓存根目录保存在注册表，使用平台原生通知监听其变化。
pub fn watch_registry_settings(on_change: Arc<dyn Fn() + Send + Sync>) {
    if let Err(error) = qq_music::watch_cache_path_changes(on_change) {
        log::warn!("监听 QQ 音乐缓存目录设置失败: {error}");
    }
}

#[cfg(test)]
mod tests {
    use crate::media::MediaPlayer;

    use super::{
        LyricsLookupMiss, LyricsLookupOutcome, TrackDescriptor, completed_lookup,
        resolve_current_local,
    };

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
    fn completed_empty_lookup_is_normal_miss() {
        let outcome = completed_lookup(None);

        assert!(matches!(
            outcome,
            LyricsLookupOutcome::Miss(LyricsLookupMiss::NoReliableLyrics)
        ));
    }

    #[test]
    fn unsupported_local_capability_is_explicit() {
        let outcome = resolve_current_local(&track(MediaPlayer::SodaMusic), None);

        assert!(matches!(outcome, Ok(LyricsLookupOutcome::Unsupported)));
    }

    #[test]
    fn missing_supported_local_data_is_normal_miss() {
        let outcome = resolve_current_local(&track(MediaPlayer::QqMusic), None);

        assert!(matches!(
            outcome,
            Ok(LyricsLookupOutcome::Miss(LyricsLookupMiss::DataUnavailable))
        ));
    }
}
