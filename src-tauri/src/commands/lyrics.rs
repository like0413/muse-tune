use tauri::State;

use crate::lyrics::{LyricsDiagnostics, LyricsService, LyricsSnapshot};

/// 返回最近一次歌词解析状态，供新创建的任务栏窗口补取初始值。
#[tauri::command]
pub fn get_current_lyrics(service: State<'_, LyricsService>) -> LyricsSnapshot {
    service.snapshot()
}

/// 返回设置页所需的只读歌词诊断信息。
#[tauri::command]
pub fn get_lyrics_diagnostics(service: State<'_, LyricsService>) -> LyricsDiagnostics {
    service.diagnostics()
}

/// 同步歌词总开关到后端解析生命周期。
#[tauri::command]
pub fn set_lyrics_enabled(enabled: bool, service: State<'_, LyricsService>) -> Result<(), String> {
    service.set_enabled(enabled)
}
