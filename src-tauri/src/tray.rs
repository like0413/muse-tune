//! 应用托盘承载全部原生菜单：应用级操作直接执行，播放器开关转发给任务栏窗口。

use tauri::{
    App, AppHandle, Emitter, Manager, Wry,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

use crate::commands;

const TRAY_ID: &str = "main";
const SETTINGS_MENU_ID: &str = "tray-settings";
const RESTART_MENU_ID: &str = "tray-restart";
const EXIT_MENU_ID: &str = "tray-exit";
const NORMAL_COVER_MENU_ID: &str = "tray-toggle-normal-cover";
const LYRICS_COVER_MENU_ID: &str = "tray-toggle-lyrics-cover";
const LYRICS_MENU_ID: &str = "tray-toggle-lyrics";
const SPECTRUM_MENU_ID: &str = "tray-toggle-spectrum";

/// 托盘菜单动作标识，与前端任务栏窗口约定的取值保持一致。
const NORMAL_COVER_ACTION: &str = "normal-cover";
const LYRICS_COVER_ACTION: &str = "lyrics-cover";
const LYRICS_ACTION: &str = "lyrics";
const SPECTRUM_ACTION: &str = "spectrum";
/// 把托盘菜单动作投递给任务栏窗口的事件名。
const TASKBAR_MENU_ACTION_EVENT: &str = "tray://taskbar-menu-action";

const TASKBAR_PRIMARY_WINDOW_LABEL: &str = "taskbar";
const TASKBAR_WINDOW_LABEL_PREFIX: &str = "taskbar-";

/// 托盘菜单文案与勾选状态；只由任务栏窗口按当前语言和设置推送。
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrayMenuPresentation {
    labels: TrayMenuLabels,
    checked: TrayMenuChecked,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrayMenuLabels {
    normal_cover: String,
    lyrics_cover: String,
    lyrics: String,
    spectrum: String,
    settings: String,
    restart: String,
    quit: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrayMenuChecked {
    normal_cover: bool,
    lyrics_cover: bool,
    lyrics: bool,
    spectrum: bool,
}

/// 托盘菜单资源句柄；文案与勾选状态都在同一批原生菜单项上原地更新。
pub(crate) struct TrayMenu {
    normal_cover: CheckMenuItem<Wry>,
    lyrics_cover: CheckMenuItem<Wry>,
    lyrics: CheckMenuItem<Wry>,
    spectrum: CheckMenuItem<Wry>,
    settings: MenuItem<Wry>,
    restart: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

impl TrayMenu {
    /// 应用任务栏窗口推送的文案与勾选状态。
    pub(crate) fn apply(&self, presentation: TrayMenuPresentation) {
        let TrayMenuPresentation { labels, checked } = presentation;
        apply_result(
            self.normal_cover.set_text(labels.normal_cover),
            "普通模式封面",
        );
        apply_result(
            self.lyrics_cover.set_text(labels.lyrics_cover),
            "歌词模式封面",
        );
        apply_result(self.lyrics.set_text(labels.lyrics), "歌词");
        apply_result(self.spectrum.set_text(labels.spectrum), "频谱");
        apply_result(self.settings.set_text(labels.settings), "设置");
        apply_result(self.restart.set_text(labels.restart), "重启应用");
        apply_result(self.quit.set_text(labels.quit), "退出");
        apply_result(
            self.normal_cover.set_checked(checked.normal_cover),
            "普通模式封面",
        );
        apply_result(
            self.lyrics_cover.set_checked(checked.lyrics_cover),
            "歌词模式封面",
        );
        apply_result(self.lyrics.set_checked(checked.lyrics), "歌词");
        apply_result(self.spectrum.set_checked(checked.spectrum), "频谱");
    }
}

/// 创建单个常驻托盘图标，并注册应用级操作菜单。
///
/// 菜单文案先使用简体中文，任务栏窗口就绪后会按当前界面语言覆盖。
pub(super) fn initialize(app: &App) -> tauri::Result<()> {
    let normal_cover = CheckMenuItem::with_id(
        app,
        NORMAL_COVER_MENU_ID,
        "普通模式封面",
        true,
        true,
        None::<&str>,
    )?;
    let lyrics_cover = CheckMenuItem::with_id(
        app,
        LYRICS_COVER_MENU_ID,
        "歌词模式封面",
        true,
        true,
        None::<&str>,
    )?;
    let lyrics = CheckMenuItem::with_id(app, LYRICS_MENU_ID, "开启歌词", true, true, None::<&str>)?;
    let spectrum =
        CheckMenuItem::with_id(app, SPECTRUM_MENU_ID, "显示频谱", true, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let settings_item = MenuItem::with_id(app, SETTINGS_MENU_ID, "打开设置", true, None::<&str>)?;
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
    let exit_item = MenuItem::with_id(app, EXIT_MENU_ID, "退出应用", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &normal_cover,
            &lyrics_cover,
            &lyrics,
            &spectrum,
            &separator,
            &settings_item,
            &restart_item,
            &exit_item,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Muse Tune")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()));
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;

    app.manage(TrayMenu {
        normal_cover,
        lyrics_cover,
        lyrics,
        spectrum,
        settings: settings_item,
        restart: restart_item,
        quit: exit_item,
    });

    Ok(())
}

/// 分发托盘菜单事件：应用级操作就地执行，播放器开关交给任务栏窗口。
fn handle_menu_event(app: &AppHandle, menu_id: &str) {
    match menu_id {
        SETTINGS_MENU_ID => open_settings(app.clone()),
        RESTART_MENU_ID => commands::system::restart_application(app.clone()),
        EXIT_MENU_ID => app.exit(0),
        NORMAL_COVER_MENU_ID => dispatch_menu_action(app, NORMAL_COVER_ACTION),
        LYRICS_COVER_MENU_ID => dispatch_menu_action(app, LYRICS_COVER_ACTION),
        LYRICS_MENU_ID => dispatch_menu_action(app, LYRICS_ACTION),
        SPECTRUM_MENU_ID => dispatch_menu_action(app, SPECTRUM_ACTION),
        _ => {}
    }
}

/// 把菜单动作投递给唯一一个任务栏窗口，避免多显示器下重复切换同一项设置。
fn dispatch_menu_action(app: &AppHandle, action: &str) {
    let window = app
        .get_webview_window(TASKBAR_PRIMARY_WINDOW_LABEL)
        .or_else(|| {
            app.webview_windows()
                .into_iter()
                .find(|(label, _)| label.starts_with(TASKBAR_WINDOW_LABEL_PREFIX))
                .map(|(_, window)| window)
        });
    let Some(window) = window else {
        log::warn!("没有可执行托盘菜单动作的任务栏窗口: {action}");
        return;
    };
    if let Err(error) = window.emit(TASKBAR_MENU_ACTION_EVENT, action) {
        log::warn!("投递托盘菜单动作失败: {error}");
    }
}

/// 菜单项更新失败只记录日志，不中断其余菜单项的同步。
fn apply_result(result: tauri::Result<()>, item: &str) {
    if let Err(error) = result {
        log::warn!("更新托盘菜单项 {item} 失败: {error}");
    }
}

/// 异步打开设置窗口，避免在托盘菜单回调中阻塞主线程。
fn open_settings(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = commands::settings::open_settings_window(app).await {
            log::error!("从托盘打开设置窗口失败: {error}");
        }
    });
}
