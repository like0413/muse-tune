//! 本地优先的四播放器歌词服务；播放器私有格式严格隔离在各自模块。

mod cache;
mod error;
mod matcher;
mod model;
mod network;
mod parser;
mod players;
mod service;
pub(crate) mod settings;
mod track;
mod watcher;

pub use model::LyricsSnapshot;
pub use service::LyricsService;

/// 初始化歌词服务；媒体服务随后通过托管状态通知歌曲变化。
pub fn initialize<R: tauri::Runtime>(app: &tauri::App<R>) -> Result<LyricsService, std::io::Error> {
    LyricsService::initialize(app)
}
