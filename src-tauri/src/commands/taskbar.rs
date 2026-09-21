use tauri::WebviewWindow;

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

/// 把独立音量悬浮窗定位到触发按钮上方并显示。
#[tauri::command]
pub fn show_volume_popup(
    window: WebviewWindow,
    anchor_center_x: f64,
    theme_color: String,
) -> Result<(), IpcError> {
    taskbar::show_volume_popup(&window, anchor_center_x, theme_color)
        .map_err(|error| IpcError::new("taskbar.show-volume-popup", error, false))
}

/// 在离场动画完成后隐藏音量悬浮窗。
#[tauri::command]
pub fn hide_volume_popup(window: WebviewWindow, generation: u64) -> Result<(), IpcError> {
    taskbar::hide_volume_popup(&window, generation)
        .map_err(|error| IpcError::new("taskbar.hide-volume-popup", error, false))
}
