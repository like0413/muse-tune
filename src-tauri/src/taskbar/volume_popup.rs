//! 管理超出 bar 高度的独立音量悬浮窗及其原生所有者关系。

use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicU64, Ordering},
};

use tauri::{Emitter, Manager, PhysicalPosition, Runtime, WebviewWindow};
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GWLP_HWNDPARENT, GetWindowLongPtrW, SW_HIDE, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE,
        WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    },
};

use crate::error::Error;

const POPUP_WINDOW_LABEL: &str = "volume-popup";
const POPUP_OPEN_EVENT: &str = "volume://popup-open";
const POPUP_GAP_LOGICAL: f64 = 4.0;
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
static VISIBLE_POPUP: LazyLock<Mutex<Option<VisiblePopup>>> = LazyLock::new(|| Mutex::new(None));

#[derive(Clone, Copy)]
struct VisiblePopup {
    popup: isize,
    owner: isize,
    generation: u64,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PopupOpenPayload {
    owner_label: String,
    theme_color: String,
    generation: u64,
}

/// 定位、挂接并显示音量窗；坐标按触发 bar 的实际 DPI 转换。
pub(super) fn show<R: Runtime>(
    source: &WebviewWindow<R>,
    anchor_center_x: f64,
    theme_color: String,
) -> Result<(), Error> {
    if !source.label().starts_with(super::TASKBAR_WINDOW_LABEL) {
        return Err(Error::Message(
            "只有任务栏播放器可以打开音量悬浮窗".to_owned(),
        ));
    }
    let popup = source
        .app_handle()
        .get_webview_window(POPUP_WINDOW_LABEL)
        .ok_or_else(|| Error::Message("音量悬浮窗尚未创建".to_owned()))?;
    let scale = source.scale_factor()?;
    let popup_hwnd = popup.hwnd()?;
    let popup_config = source
        .app_handle()
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == POPUP_WINDOW_LABEL)
        .ok_or_else(|| Error::Message("找不到音量悬浮窗配置".to_owned()))?;
    apply_configured_size(popup_hwnd, scale, popup_config.width, popup_config.height)?;
    let source_position = source.outer_position()?;
    let source_size = source.outer_size()?;
    let popup_size = popup.outer_size()?;
    let monitor = source
        .current_monitor()?
        .ok_or_else(|| Error::Message("无法确定任务栏所在显示器".to_owned()))?;
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();

    let raw_x =
        source_position.x as f64 + anchor_center_x * scale - f64::from(popup_size.width) / 2.0;
    let min_x = monitor_position.x;
    let max_x = monitor_position.x + monitor_size.width as i32 - popup_size.width as i32;
    let x = (raw_x.round() as i32).clamp(min_x, max_x.max(min_x));
    let monitor_mid_y = monitor_position.y + monitor_size.height as i32 / 2;
    let source_mid_y = source_position.y + source_size.height as i32 / 2;
    let gap = (POPUP_GAP_LOGICAL * scale).round() as i32;
    let y = if source_mid_y < monitor_mid_y {
        source_position.y + source_size.height as i32 + gap
    } else {
        source_position.y - popup_size.height as i32 - gap
    };

    popup.set_position(PhysicalPosition::new(x, y))?;
    attach_to_owner(&popup, source)?;
    let owner_hwnd = source.hwnd()?;
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut visible) = VISIBLE_POPUP.lock() {
        *visible = Some(VisiblePopup {
            popup: popup_hwnd.0 as isize,
            owner: owner_hwnd.0 as isize,
            generation,
        });
    }
    popup.show()?;
    popup.emit(
        POPUP_OPEN_EVENT,
        PopupOpenPayload {
            owner_label: source.label().to_owned(),
            theme_color,
            generation,
        },
    )?;
    Ok(())
}

/// 在前端离场动画结束后隐藏悬浮窗。
pub(super) fn hide<R: Runtime>(source: &WebviewWindow<R>, generation: u64) -> Result<(), Error> {
    if source.label() != POPUP_WINDOW_LABEL {
        return Err(Error::Message("只有音量悬浮窗可以隐藏自身".to_owned()));
    }
    let hwnd = source.hwnd()?;
    let should_hide = VISIBLE_POPUP.lock().is_ok_and(|mut visible| {
        let matches = visible
            .is_some_and(|state| state.popup == hwnd.0 as isize && state.generation == generation);
        if matches {
            *visible = None;
        }
        matches
    });
    if should_hide {
        source.hide()?;
    }
    Ok(())
}

/// bar 因全屏或可见性策略隐藏时同步关闭其悬浮窗，防止恢复后意外重现。
pub(super) fn hide_for_owner(owner: HWND) {
    let popup = VISIBLE_POPUP
        .lock()
        .ok()
        .and_then(|mut visible| match *visible {
            Some(state) if state.owner == owner.0 as isize => {
                *visible = None;
                Some(state.popup)
            }
            _ => None,
        });
    if let Some(popup) = popup {
        hide_native(HWND(popup as *mut _));
    }
}

/// 建立 no-activate 工具窗样式，并让 popup 随所属 bar 隐藏。
fn attach_to_owner<R: Runtime>(
    popup: &WebviewWindow<R>,
    owner: &WebviewWindow<R>,
) -> Result<(), Error> {
    let popup_hwnd = popup.hwnd()?;
    let owner_hwnd = owner.hwnd()?;
    // SAFETY: 两个句柄均来自存活的 Tauri 顶层窗口，仅调整 owner 与扩展样式。
    unsafe {
        let mut style = WINDOW_EX_STYLE(GetWindowLongPtrW(popup_hwnd, GWL_EXSTYLE) as u32);
        style |= WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        style &= !WS_EX_APPWINDOW;
        SetWindowLongPtrW(popup_hwnd, GWL_EXSTYLE, style.0 as isize);
        SetWindowLongPtrW(popup_hwnd, GWLP_HWNDPARENT, owner_hwnd.0 as isize);
    }
    Ok(())
}

/// 将 Tauri 配置的逻辑尺寸同步到原生 popup，避免普通顶层窗口的最小宽度改变配置语义。
fn apply_configured_size(
    window: HWND,
    scale: f64,
    logical_width: f64,
    logical_height: f64,
) -> Result<(), Error> {
    if !scale.is_finite()
        || scale <= 0.0
        || !logical_width.is_finite()
        || logical_width <= 0.0
        || !logical_height.is_finite()
        || logical_height <= 0.0
    {
        return Err(Error::Message("音量悬浮窗尺寸配置无效".to_owned()));
    }
    let width = (logical_width * scale).round() as i32;
    let height = (logical_height * scale).round() as i32;
    // SAFETY: 句柄来自存活的 Tauri popup；只设置已验证的正尺寸，不移动、不激活窗口。
    unsafe {
        SetWindowPos(
            window,
            None,
            0,
            0,
            width,
            height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }?;
    Ok(())
}

/// 使用 Win32 隐藏窗口，保持无激活行为。
fn hide_native(window: HWND) {
    // SAFETY: 句柄只来自当前仍注册的 Tauri popup；失效句柄下 ShowWindow 也会安全失败。
    let _ = unsafe { ShowWindow(window, SW_HIDE) };
}
