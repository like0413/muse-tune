use tauri::State;

use crate::system::SystemThemeService;

/// 枚举 Windows 当前安装的字体族，供歌词字体搜索选择器使用。
#[tauri::command]
pub async fn list_system_fonts() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(crate::system::list_system_fonts)
        .await
        .map_err(|error| format!("等待系统字体枚举失败: {error}"))?
        .map_err(|error| format!("枚举系统字体失败: {error}"))
}

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
