//! 应用托盘只负责菜单生命周期和菜单事件分发。

use tauri::{
    App,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

use crate::commands;

const SETTINGS_MENU_ID: &str = "tray-settings";
const RESTART_MENU_ID: &str = "tray-restart";
const EXIT_MENU_ID: &str = "tray-exit";

/// 创建单个常驻托盘图标，并注册应用级操作菜单。
pub(super) fn initialize(app: &App) -> tauri::Result<()> {
    let settings_item = MenuItem::with_id(app, SETTINGS_MENU_ID, "设置", true, None::<&str>)?;
    let restart_item = MenuItem::with_id(
        app,
        RESTART_MENU_ID,
        if cfg!(debug_assertions) {
            "重启应用（正式版可用）"
        } else {
            "重启应用"
        },
        !cfg!(debug_assertions),
        None::<&str>,
    )?;
    let exit_item = MenuItem::with_id(app, EXIT_MENU_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings_item, &restart_item, &exit_item])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("Muse Tune")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            SETTINGS_MENU_ID => open_settings(app.clone()),
            RESTART_MENU_ID => commands::system::restart_application(app.clone()),
            EXIT_MENU_ID => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;

    Ok(())
}

/// 异步打开设置窗口，避免在托盘菜单回调中阻塞主线程。
fn open_settings(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = commands::settings::open_settings_window(app).await {
            log::error!("从托盘打开设置窗口失败: {error}");
        }
    });
}
