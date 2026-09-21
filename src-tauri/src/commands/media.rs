use tauri::State;

use crate::{
    commands::run_blocking,
    ipc::IpcError,
    media::{
        MediaControlAction, MediaService, MediaSessionSelectionPolicy, MediaSessionSnapshot,
        MediaVolumeSnapshot,
    },
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
) -> Result<bool, IpcError> {
    let service = service.inner().clone();
    run_blocking(
        "media.control-session",
        "等待媒体控制结果失败",
        move || service.control(action),
    )
    .await
}

#[tauri::command]
pub async fn toggle_current_media_player(service: State<'_, MediaService>) -> Result<(), IpcError> {
    let service = service.inner().clone();
    run_blocking(
        "media.toggle-player-window",
        "等待播放器窗口开关结果失败",
        move || service.toggle_player_window(),
    )
    .await
}

/// 更新多播放器会话选择策略，并在媒体线程内串行应用。
#[tauri::command]
pub async fn set_media_session_selection_policy(
    policy: MediaSessionSelectionPolicy,
    service: State<'_, MediaService>,
) -> Result<(), IpcError> {
    let service = service.inner().clone();
    run_blocking(
        "media.set-selection-policy",
        "等待媒体会话策略更新失败",
        move || service.set_selection_policy(policy),
    )
    .await
}

#[tauri::command]
pub async fn get_current_media_volume(
    service: State<'_, MediaService>,
) -> Result<Option<MediaVolumeSnapshot>, IpcError> {
    let service = service.inner().clone();
    run_blocking("media.get-volume", "等待应用音量失败", move || {
        service.volume()
    })
    .await
}

#[tauri::command]
pub async fn set_current_media_volume(
    level: f32,
    service: State<'_, MediaService>,
) -> Result<MediaVolumeSnapshot, IpcError> {
    if !level.is_finite() {
        return Err(IpcError::new("media.set-volume", "音量值无效", false));
    }
    let service = service.inner().clone();
    run_blocking(
        "media.set-volume",
        "等待应用音量设置结果失败",
        move || service.set_volume(level),
    )
    .await
}

#[tauri::command]
pub async fn toggle_current_media_mute(
    service: State<'_, MediaService>,
) -> Result<MediaVolumeSnapshot, IpcError> {
    let service = service.inner().clone();
    run_blocking(
        "media.toggle-mute",
        "等待应用静音切换结果失败",
        move || service.toggle_mute(),
    )
    .await
}

/// 启用或停止当前播放器的按进程音频频谱采集。
#[tauri::command]
pub async fn set_media_spectrum_enabled(
    enabled: bool,
    frame_rate: u16,
    service: State<'_, MediaService>,
) -> Result<(), IpcError> {
    let service = service.inner().clone();
    run_blocking(
        "media.set-spectrum-enabled",
        "等待频谱开关结果失败",
        move || service.set_spectrum_enabled(enabled, frame_rate),
    )
    .await
}
