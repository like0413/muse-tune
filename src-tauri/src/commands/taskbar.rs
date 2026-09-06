use crate::taskbar::{self, TaskbarDisplay, TaskbarOverlapPriority, TaskbarPlacement};

/// 枚举当前拥有任务栏的显示器。
#[tauri::command]
pub fn list_taskbar_displays() -> Vec<TaskbarDisplay> {
    taskbar::available_displays()
}

/// 更新 bar 应显示的目标显示器。
#[tauri::command]
pub fn set_taskbar_display_target(target: String) -> Result<(), String> {
    taskbar::set_display_target(target)
}

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

/// 更新 bar 基准宽度，并立即唤醒所有任务栏同步线程。
#[tauri::command]
pub fn set_taskbar_width(width: i32) -> Result<(), String> {
    taskbar::set_content_width(width);
    Ok(())
}
