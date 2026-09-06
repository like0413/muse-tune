use crate::taskbar::{self, TaskbarOverlapPriority, TaskbarPlacement};

/// 更新播放器定位偏好，并立即唤醒任务栏同步线程。
#[tauri::command]
pub fn set_taskbar_placement(placement: TaskbarPlacement) -> Result<(), String> {
    taskbar::set_placement(placement);
    Ok(())
}

/// 更新任务栏元素与播放器的遮挡优先级，并立即唤醒任务栏同步线程。
#[tauri::command]
pub fn set_taskbar_overlap_priority(priority: TaskbarOverlapPriority) -> Result<(), String> {
    taskbar::set_overlap_priority(priority);
    Ok(())
}
