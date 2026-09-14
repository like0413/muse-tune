//! 通过 Windows GSMTC 事件提供当前歌曲信息和基础播放控制。

use std::sync::Arc;

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
    MediaControlAction, MediaPlaybackStatus, MediaPlayer, MediaSessionSelectionPolicy,
    MediaSessionSelectionStrategy, MediaSessionSnapshot, MediaTimeline, MediaVolumeSnapshot,
};
pub(crate) use model::{
    MediaRuntimeDiagnostics, MediaRuntimeSessionDiagnostics, MediaSnapshotDiagnostics,
};
pub use monitor::MediaService;

/// 完整媒体快照的后端订阅者，由组合根注入以隔离下游领域实现。
pub(crate) type MediaSnapshotSubscriber = Arc<dyn Fn(&Option<MediaSessionSnapshot>) + Send + Sync>;

/// 启动媒体会话服务。
pub(crate) fn initialize<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    snapshot_subscriber: MediaSnapshotSubscriber,
) -> Result<MediaService, std::io::Error> {
    MediaService::initialize(app, snapshot_subscriber)
}

use model::{MediaMetadata, MediaPlayback, MediaPlaybackControls};
