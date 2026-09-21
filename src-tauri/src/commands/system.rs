use tauri::{AppHandle, State};

use crate::{commands::run_blocking, error::Error, ipc::IpcError, system::SystemThemeService};

/// 走 Tauri 的退出流程重启：`lib.rs` 在带退出码的 `ExitRequested` 与 `Exit` 事件里停止媒体、
/// 任务栏、歌词与主题服务，直接结束进程会跳过这些回收。
#[tauri::command]
pub fn restart_application(app: AppHandle) {
    app.request_restart();
}

/// 枚举 Windows 当前安装的字体族，供歌词字体搜索选择器使用。
#[tauri::command]
pub async fn list_system_fonts() -> Result<Vec<String>, IpcError> {
    run_blocking(
        "system.list-fonts",
        "等待系统字体枚举失败",
        || {
            // 枚举本身的失败与等待失败分开描述，便于区分是线程池还是字体接口出问题。
            crate::system::list_system_fonts()
                .map_err(|error| Error::Message(format!("枚举系统字体失败: {error}")))
        },
    )
    .await
}

/// 读取 Windows 当前强调色，供任务栏首次渲染使用。
#[tauri::command]
pub fn get_system_accent_color(service: State<'_, SystemThemeService>) -> Result<String, IpcError> {
    service
        .accent_color()
        .map_err(|error| IpcError::new("system.get-accent-color", error, true))
}

/// 读取 Windows 当前前景色，供高透明 bar 保持文字对比度。
#[tauri::command]
pub fn get_system_foreground_color(
    service: State<'_, SystemThemeService>,
) -> Result<String, IpcError> {
    service
        .foreground_color()
        .map_err(|error| IpcError::new("system.get-foreground-color", error, true))
}
