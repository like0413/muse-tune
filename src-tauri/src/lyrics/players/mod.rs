mod kugou_music;
mod netease_cloud_music;
mod qq_music;
mod soda_music;

use std::{path::PathBuf, sync::Arc};

use reqwest::blocking::Client;

use crate::media::MediaPlayer;

use super::{error::LyricsError, model::ResolvedLyrics, track::TrackDescriptor};

/// 歌词功能首版支持的播放器集合，供设置与监听复用。
pub const SUPPORTED_PLAYERS: [MediaPlayer; 4] = [
    MediaPlayer::QqMusic,
    MediaPlayer::NeteaseCloudMusic,
    MediaPlayer::SodaMusic,
    MediaPlayer::KugouMusic,
];

/// 按当前播放器进入完全隔离的平台解析入口。
pub fn resolve_current_player(
    track: &TrackDescriptor,
    cache_path: Option<PathBuf>,
    client: &Client,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    match track.player {
        MediaPlayer::QqMusic => qq_music::resolve(track, cache_path.as_deref(), client),
        MediaPlayer::NeteaseCloudMusic => {
            netease_cloud_music::resolve(track, cache_path.as_deref(), client)
        }
        MediaPlayer::SodaMusic => soda_music::resolve(track, cache_path.as_deref(), client),
        MediaPlayer::KugouMusic => kugou_music::resolve(track, cache_path.as_deref()),
        MediaPlayer::Other => Ok(None),
    }
}

/// 使用 QQ 音乐 HTTPS 作为跨播放器兜底。
pub fn resolve_qq_online(
    track: &TrackDescriptor,
    client: &Client,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    qq_music::resolve_online(track, client)
}

/// 仅查询 QQ 音乐本地 QRC，供协调器为其他播放器补足真实逐字时间轴。
pub fn resolve_qq_local(
    track: &TrackDescriptor,
    cache_path: Option<PathBuf>,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let Some(cache_path) = cache_path else {
        return Ok(None);
    };
    qq_music::resolve_local(track, &cache_path)
}

/// 使用网易云音乐 HTTPS 作为跨播放器兜底。
pub fn resolve_netease_online(
    track: &TrackDescriptor,
    client: &Client,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    netease_cloud_music::resolve_online(track, client)
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

/// 返回单个平台需要通过文件系统事件观察的配置目录。
pub fn configuration_watch_paths(player: MediaPlayer) -> Vec<PathBuf> {
    match player {
        MediaPlayer::KugouMusic => kugou_music::configuration_watch_path()
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

/// QQ 的缓存根目录保存在注册表，使用平台原生通知监听其变化。
pub fn watch_registry_settings(on_change: Arc<dyn Fn() + Send + Sync>) {
    if let Err(error) = qq_music::watch_cache_path_changes(on_change) {
        log::warn!("监听 QQ 音乐缓存目录设置失败: {error}");
    }
}
