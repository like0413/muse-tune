use crate::{
    ipc::IpcError,
    taskbar::{self, TaskbarDisplay, TaskbarOverlapPriority, TaskbarPlacement, TaskbarWidthMode},
};

/// 枚举当前拥有任务栏的显示器。
#[tauri::command]
pub fn list_taskbar_displays() -> Vec<TaskbarDisplay> {
    taskbar::available_displays()
}

/// 更新 bar 应显示的目标显示器。
#[tauri::command]
pub fn set_taskbar_display_target(target: String) -> Result<(), IpcError> {
    taskbar::set_display_target(target)
        .map_err(|error| IpcError::new("taskbar.set-display-target", error, false))
}

#[tauri::command]
pub fn set_taskbar_placement(placement: TaskbarPlacement) {
    taskbar::set_placement(placement);
}

#[tauri::command]
/// 立即更新所有任务栏窗口的有符号水平偏移。
pub fn set_taskbar_horizontal_offset(offset: i32) {
    taskbar::set_horizontal_offset(offset);
}

#[tauri::command]
pub fn set_taskbar_overlap_priority(priority: TaskbarOverlapPriority) {
    taskbar::set_overlap_priority(priority);
}

#[tauri::command]
pub fn set_taskbar_width(width: i32, mode: TaskbarWidthMode) {
    taskbar::set_content_width(width);
    taskbar::set_width_mode(mode);
}

#[tauri::command]
pub fn set_taskbar_content_visibility(visible: bool) {
    taskbar::set_content_visibility(visible);
}
