use tauri::State;

use crate::media::{
    MediaControlAction, MediaService, MediaSessionSelectionPolicy, MediaSessionSnapshot,
    MediaVolumeSnapshot,
};

/// 返回最近一次由 Windows 媒体会话事件刷新出的完整快照。
#[tauri::command]
pub fn get_current_media_session(service: State<'_, MediaService>) -> Option<MediaSessionSnapshot> {
    service.snapshot()
}

/// 向 Windows 当前媒体会话发送播放控制请求。
#[tauri::command]
pub async fn control_media_session(
    action: MediaControlAction,
    service: State<'_, MediaService>,
) -> Result<bool, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.control(action))
        .await
        .map_err(|error| format!("等待媒体控制结果失败: {error}"))?
}

/// 开关当前媒体会话所属的播放器主窗口：已在前台时关闭，最小化或隐藏到托盘时还原或显示。
#[tauri::command]
pub async fn toggle_current_media_player(service: State<'_, MediaService>) -> Result<(), String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.toggle_player_window())
        .await
        .map_err(|error| format!("等待播放器窗口开关结果失败: {error}"))?
}

/// 更新多播放器会话选择策略，并在媒体线程内串行应用。
#[tauri::command]
pub async fn set_media_session_selection_policy(
    policy: MediaSessionSelectionPolicy,
    service: State<'_, MediaService>,
) -> Result<(), String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.set_selection_policy(policy))
        .await
        .map_err(|error| format!("等待媒体会话策略更新失败: {error}"))?
}

/// 返回当前播放器的 Windows 单应用音量，不读取系统全局音量。
#[tauri::command]
pub async fn get_current_media_volume(
    service: State<'_, MediaService>,
) -> Result<Option<MediaVolumeSnapshot>, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.volume())
        .await
        .map_err(|error| format!("等待应用音量失败: {error}"))?
}

/// 设置当前播放器的 Windows 单应用音量。
#[tauri::command]
pub async fn set_current_media_volume(
    level: f32,
    service: State<'_, MediaService>,
) -> Result<MediaVolumeSnapshot, String> {
    if !level.is_finite() {
        return Err("音量值无效".to_owned());
    }
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.set_volume(level))
        .await
        .map_err(|error| format!("等待应用音量设置结果失败: {error}"))?
}

/// 切换当前播放器的 Windows 单应用静音状态。
#[tauri::command]
pub async fn toggle_current_media_mute(
    service: State<'_, MediaService>,
) -> Result<MediaVolumeSnapshot, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.toggle_mute())
        .await
        .map_err(|error| format!("等待应用静音切换结果失败: {error}"))?
}

/// 启用或停止当前播放器的按进程音频频谱采集。
#[tauri::command]
pub async fn set_media_spectrum_enabled(
    enabled: bool,
    frame_rate: u16,
    service: State<'_, MediaService>,
) -> Result<(), String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.set_spectrum_enabled(enabled, frame_rate))
        .await
        .map_err(|error| format!("等待频谱开关结果失败: {error}"))?
}
