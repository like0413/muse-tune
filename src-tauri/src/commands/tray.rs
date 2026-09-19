use tauri::State;

use crate::tray::{TrayMenu, TrayMenuPresentation};

/// 接收任务栏窗口推送的托盘菜单文案与勾选状态。
#[tauri::command]
pub fn set_tray_menu_state(menu: State<'_, TrayMenu>, presentation: TrayMenuPresentation) {
    menu.apply(presentation);
}
