//! 应用托盘承载全部原生菜单：应用级操作直接执行，播放器开关转发给任务栏窗口。

use std::sync::Mutex;

use tauri::{
    App, AppHandle, Emitter, Manager, Wry,
    image::Image,
    menu::{CheckMenuItem, IconMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder},
};

use crate::commands;

const TRAY_ID: &str = "main";
const UPDATE_MENU_ID: &str = "tray-update";
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

/// 更新入口的完整展示状态；无可用版本时两个字段都为 `None`。
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateTrayPresentation {
    label: Option<String>,
    tooltip: Option<String>,
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
    menu: Menu<Wry>,
    update: IconMenuItem<Wry>,
    update_separator: PredefinedMenuItem<Wry>,
    update_label: Mutex<Option<String>>,
    tray: TrayIcon<Wry>,
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

    /// 原地同步更新入口；菜单项只在有新版本时插入，避免常态占用空间。
    pub(crate) fn apply_update(&self, presentation: UpdateTrayPresentation) {
        let Ok(mut current_label) = self.update_label.lock() else {
            log::error!("更新托盘入口状态锁已损坏");
            return;
        };

        match presentation.label {
            Some(label) => {
                if let Err(error) = self.update.set_text(&label) {
                    log::warn!("更新托盘菜单项文案失败: {error}");
                    return;
                }
                if current_label.is_none()
                    && let Err(error) = self
                        .menu
                        .prepend_items(&[&self.update, &self.update_separator])
                {
                    log::warn!("显示托盘更新入口失败: {error}");
                    return;
                }
                if let Some(tooltip) = presentation.tooltip {
                    apply_result(self.tray.set_tooltip(Some(tooltip)), "托盘更新提示");
                }
                *current_label = Some(label);
            }
            None => {
                if current_label.take().is_some() {
                    apply_result(self.menu.remove(&self.update), "托盘更新入口");
                    apply_result(self.menu.remove(&self.update_separator), "托盘更新分隔线");
                }
                // 使用配置中的应用名，保证恢复默认提示时仍能区分开发版与正式版。
                apply_result(
                    self.tray
                        .set_tooltip(Some(&self.tray.app_handle().package_info().name)),
                    "托盘默认提示",
                );
            }
        }
    }
}

/// 生成 16px 高对比红色下载图标，不依赖额外图片解码能力或磁盘资源。
fn update_menu_icon() -> Image<'static> {
    const SIZE: usize = 16;
    let mut rgba = vec![0_u8; SIZE * SIZE * 4];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - 7.5;
            let dy = y as f32 - 7.5;
            if dx * dx + dy * dy <= 49.0 {
                let offset = (y * SIZE + x) * 4;
                rgba[offset..offset + 4].copy_from_slice(&[255, 59, 48, 255]);
            }
        }
    }

    for &(x, y) in &[
        (7, 3),
        (8, 3),
        (7, 4),
        (8, 4),
        (7, 5),
        (8, 5),
        (7, 6),
        (8, 6),
        (7, 7),
        (8, 7),
        (5, 8),
        (6, 8),
        (7, 8),
        (8, 8),
        (9, 8),
        (10, 8),
        (6, 9),
        (7, 9),
        (8, 9),
        (9, 9),
        (7, 10),
        (8, 10),
        (7, 11),
        (8, 11),
    ] {
        let offset = (y * SIZE + x) * 4;
        rgba[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
    }

    Image::new_owned(rgba, SIZE as u32, SIZE as u32)
}

/// 创建单个常驻托盘图标，并注册应用级操作菜单。
///
/// 菜单文案先使用简体中文，任务栏窗口就绪后会按当前界面语言覆盖。
pub(super) fn initialize(app: &App) -> tauri::Result<()> {
    let update_item = IconMenuItem::with_id(
        app,
        UPDATE_MENU_ID,
        "发现新版本",
        true,
        Some(update_menu_icon()),
        None::<&str>,
    )?;
    let update_separator = PredefinedMenuItem::separator(app)?;
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
        .tooltip(&app.package_info().name)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()));
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;

    app.manage(TrayMenu {
        menu,
        update: update_item,
        update_separator,
        update_label: Mutex::new(None),
        tray,
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
        UPDATE_MENU_ID => open_update_settings(app.clone()),
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

/// 异步打开设置窗口的“关于”页，供更新入口直达安装操作。
fn open_update_settings(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = commands::settings::open_update_settings_window(&app) {
            log::error!("从托盘更新入口打开设置窗口失败: {error}");
        }
    });
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
