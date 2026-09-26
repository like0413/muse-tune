use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{error::Error, ipc::IpcError, storage::StoragePaths};

const SETTINGS_WINDOW_LABEL: &str = "settings";
const SETTINGS_SECTION_EVENT: &str = "settings://select-section";
const ABOUT_SECTION: &str = "about";
/// 打开设置窗口失败时返回给前端的命令码。
const OPEN_WINDOW_CODE: &str = "settings.open-window";

/// 原生入口要求设置窗口执行的导航动作。
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsNavigation {
    section: &'static str,
    automatic_install: bool,
}

/// 创建设置窗口，或唤醒已经就绪的设置窗口。
///
/// 必须保持异步 command，避免 Windows WebView2 在同步 command 中创建窗口时死锁。
#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), IpcError> {
    open_or_activate_settings_window(&app, None)
        .map_err(|error| IpcError::new(OPEN_WINDOW_CODE, error, true))
}

/// 从原生更新入口打开设置窗口，并直接定位到“关于”页。
pub(crate) fn open_update_settings_window(app: &AppHandle) -> Result<(), Error> {
    open_or_activate_settings_window(
        app,
        Some(SettingsNavigation {
            section: ABOUT_SECTION,
            automatic_install: true,
        }),
    )
}

/// 打开设置窗口的实现；命令层只负责把失败收敛为 IPC 错误。
fn open_or_activate_settings_window(
    app: &AppHandle,
    navigation: Option<SettingsNavigation>,
) -> Result<(), Error> {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        activate_settings_window_if_ready(&window)?;
        if let Some(navigation) = navigation {
            window.emit(SETTINGS_SECTION_EVENT, navigation)?;
        }
        return Ok(());
    }

    let app_config = app.config();
    let mut window_config = app_config
        .app
        .windows
        .iter()
        .find(|config| config.label == SETTINGS_WINDOW_LABEL)
        .ok_or_else(|| Error::Message("未找到设置窗口配置".to_owned()))?
        .clone();
    if let Some(navigation) = navigation {
        window_config.url = WebviewUrl::App(PathBuf::from(format!(
            "index.html#/settings?section={}&automaticInstall={}",
            navigation.section, navigation.automatic_install
        )));
    }

    let mut builder = WebviewWindowBuilder::from_config(app, &window_config)?;
    if let Some(directory) = app.state::<StoragePaths>().webview_directory() {
        builder = builder.data_directory(directory.to_path_buf());
    }

    match builder.build() {
        Ok(_) => Ok(()),
        Err(error) => {
            // 连续右键可能让另一条命令抢先完成创建，此时直接复用已存在的窗口。
            if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
                return activate_settings_window_if_ready(&window);
            }
            Err(error.into())
        }
    }
}

/// 显示设置窗口并将其带到前台。
fn activate_settings_window(window: &WebviewWindow) -> Result<(), Error> {
    window.unminimize()?;
    window.show()?;
    window.set_focus()?;
    Ok(())
}

/// 仅唤醒已经显示的窗口；隐藏状态表示页面仍在首次加载。
fn activate_settings_window_if_ready(window: &WebviewWindow) -> Result<(), Error> {
    if window.is_visible()? {
        activate_settings_window(window)?;
    }

    Ok(())
}
