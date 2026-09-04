use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder, webview::PageLoadEvent};

const SETTINGS_WINDOW_LABEL: &str = "settings";

/// 创建设置窗口，或唤醒已经就绪的设置窗口。
///
/// 必须保持异步 command，避免 Windows WebView2 在同步 command 中创建窗口时死锁。
#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        return activate_settings_window_if_ready(&window);
    }

    let app_config = app.config();
    let window_config = app_config
        .app
        .windows
        .iter()
        .find(|config| config.label == SETTINGS_WINDOW_LABEL)
        .ok_or_else(|| "未找到设置窗口配置".to_owned())?;

    let builder = WebviewWindowBuilder::from_config(&app, window_config)
        .map_err(|error| error.to_string())?
        .on_page_load(|window, payload| {
            if matches!(payload.event(), PageLoadEvent::Finished)
                && let Err(error) = show_settings_window_after_load(&window)
            {
                log::error!("显示设置窗口失败: {error}");
            }
        });

    match builder.build() {
        Ok(_) => Ok(()),
        Err(error) => {
            // 连续右键可能让另一条命令抢先完成创建，此时直接复用已存在的窗口。
            if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
                return activate_settings_window_if_ready(&window);
            }
            Err(error.to_string())
        }
    }
}

/// 显示设置窗口并将其带到前台。
fn activate_settings_window(window: &WebviewWindow) -> Result<(), String> {
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

/// 仅唤醒已经显示的窗口；隐藏状态表示页面仍在首次加载。
fn activate_settings_window_if_ready(window: &WebviewWindow) -> Result<(), String> {
    if window.is_visible().map_err(|error| error.to_string())? {
        activate_settings_window(window)?;
    }

    Ok(())
}

/// 页面首次加载完成后显示窗口，后续页面刷新不抢占用户焦点。
fn show_settings_window_after_load(window: &WebviewWindow) -> Result<(), String> {
    if !window.is_visible().map_err(|error| error.to_string())? {
        activate_settings_window(window)?;
    }

    Ok(())
}
