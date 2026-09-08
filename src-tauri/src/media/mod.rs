//! 通过 Windows GSMTC 事件提供当前歌曲信息和基础播放控制。

mod model;
mod monitor;
mod players;
mod process;
mod selector;
mod source_icon;
mod spectrum;
mod thumbnail;
mod volume;

pub use model::{
    MediaControlAction, MediaSessionSelectionPolicy, MediaSessionSnapshot, MediaTimeline,
    MediaVolumeSnapshot,
};
pub use monitor::MediaService;

/// 启动媒体会话服务。
pub fn initialize<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<MediaService, std::io::Error> {
    MediaService::initialize(app)
}

use model::{
    MediaMetadata, MediaPlayback, MediaPlaybackControls, MediaPlayer, MediaSessionSelectionStrategy,
};
