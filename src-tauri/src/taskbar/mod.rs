//! 使用 Win32 所有者窗口关系，将 Tauri bar 窗口集成到 Windows 任务栏。

mod events;
mod geometry;
mod platform;
mod sync;

use std::{thread, time::Duration};

use tauri::{AppHandle, Manager, Runtime};

const TASKBAR_WINDOW_LABEL: &str = "taskbar";
const RECOVERY_RETRY_DELAY: Duration = Duration::from_millis(400);

/// 启动独立监控线程，持续维护任务栏与播放器窗口的所有者关系。
pub fn initialize<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    thread::Builder::new()
        .name("taskbar-monitor".to_owned())
        .spawn(move || maintain_bar_window(app_handle))?;

    Ok(())
}

/// 获取或重建播放器窗口，并在 Explorer 生命周期内持续恢复同步。
fn maintain_bar_window<R: Runtime>(app: AppHandle<R>) {
    loop {
        let bar_window = if let Some(bar_window) = app.get_webview_window(TASKBAR_WINDOW_LABEL) {
            bar_window
        } else {
            let Some(config) = app
                .config()
                .app
                .windows
                .iter()
                .find(|config| config.label == TASKBAR_WINDOW_LABEL)
                .cloned()
            else {
                return;
            };

            let Ok(builder) = tauri::WebviewWindowBuilder::from_config(&app, &config) else {
                wait_before_recovery();
                continue;
            };
            let Ok(bar_window) = builder.build() else {
                wait_before_recovery();
                continue;
            };
            bar_window
        };

        let Ok(window_handle) = bar_window.hwnd() else {
            wait_before_recovery();
            continue;
        };
        sync::run(window_handle.0 as isize);
        wait_before_recovery();
    }
}

/// 在 Explorer 或窗口短暂不可用时限制重试频率。
fn wait_before_recovery() {
    thread::sleep(RECOVERY_RETRY_DELAY);
}
