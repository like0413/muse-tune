use crate::taskbar::{self, TaskbarPlacement};

/// 更新播放器定位偏好，并立即唤醒任务栏同步线程。
#[tauri::command]
pub fn set_taskbar_placement(placement: TaskbarPlacement) -> Result<(), String> {
    taskbar::set_placement(placement);
    Ok(())
}
