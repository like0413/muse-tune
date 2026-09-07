use tauri::State;

use crate::system::SystemThemeService;

/// 读取 Windows 当前强调色，供任务栏首次渲染使用。
#[tauri::command]
pub fn get_system_accent_color(service: State<'_, SystemThemeService>) -> Result<String, String> {
    service.accent_color()
}

/// 读取 Windows 当前前景色，供高透明 bar 保持文字对比度。
#[tauri::command]
pub fn get_system_foreground_color(
    service: State<'_, SystemThemeService>,
) -> Result<String, String> {
    service.foreground_color()
}
