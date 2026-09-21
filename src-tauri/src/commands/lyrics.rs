use tauri::State;

use crate::{
    ipc::IpcError,
    lyrics::{LyricsOnlineStrategy, LyricsService, LyricsSnapshot},
    media::MediaPlayer,
};

/// 返回最近一次歌词解析状态，供新创建的任务栏窗口补取初始值。
#[tauri::command]
pub fn get_current_lyrics(service: State<'_, LyricsService>) -> LyricsSnapshot {
    service.snapshot()
}

/// 同步歌词总开关、联网能力、在线调度策略与在线接口集合到后端解析生命周期。
#[tauri::command]
pub fn set_lyrics_preferences(
    enabled: bool,
    allow_online: bool,
    online_strategy: LyricsOnlineStrategy,
    online_sources: Vec<MediaPlayer>,
    service: State<'_, LyricsService>,
) -> Result<(), IpcError> {
    service
        .set_preferences(enabled, allow_online, online_strategy, online_sources)
        .map_err(|error| IpcError::new("lyrics.set-preferences", error, false))
}
