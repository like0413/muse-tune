use tauri::State;

use crate::media::{
    MediaControlAction, MediaService, MediaSessionSelectionPolicy, MediaSessionSnapshot,
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
