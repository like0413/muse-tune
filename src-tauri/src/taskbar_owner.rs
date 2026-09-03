//! 使用 Win32 所有者窗口关系，将 Tauri bar 窗口集成到 Windows 任务栏。

use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager, Runtime};
use windows::{
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
        },
        UI::{
            Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
            HiDpi::GetDpiForWindow,
            Shell::{ABM_GETSTATE, ABS_AUTOHIDE, APPBARDATA, SHAppBarMessage},
            WindowsAndMessaging::{
                DispatchMessageW, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
                EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_REORDER, EVENT_OBJECT_SHOW,
                EVENT_SYSTEM_FOREGROUND, FindWindowExW, FindWindowW, GA_ROOT, GWL_EXSTYLE,
                GWLP_HWNDPARENT, GetAncestor, GetClassNameW, GetForegroundWindow,
                GetWindowLongPtrW, GetWindowRect, HWND_TOPMOST, IsWindow, IsWindowVisible, MSG,
                MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, OBJID_WINDOW, PM_REMOVE,
                PeekMessageW, QS_ALLINPUT, SW_HIDE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER,
                SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, ShowWindow,
                TranslateMessage, WINDOW_EX_STYLE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
                WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            },
        },
    },
    core::PCWSTR,
};

const BAR_WIDTH_DIP: i32 = 360;
const BASE_DPI: u32 = 96;
const RECOVERY_RETRY_DELAY: Duration = Duration::from_millis(400);
const WINDOW_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const BAR_WINDOW_LABEL: &str = "main";
static WINDOW_STATE_CHANGED: AtomicBool = AtomicBool::new(false);
static TASKBAR_LAYOUT_CHANGED: AtomicBool = AtomicBool::new(false);

enum TaskbarChange {
    WindowState,
    Layout,
    Timeout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScreenRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl From<RECT> for ScreenRect {
    fn from(value: RECT) -> Self {
        Self {
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
        }
    }
}

impl ScreenRect {
    fn width(self) -> i32 {
        self.right - self.left
    }

    fn height(self) -> i32 {
        self.bottom - self.top
    }
}

pub fn initialize<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    thread::Builder::new()
        .name("taskbar-owner-monitor".to_owned())
        .spawn(move || maintain_bar_window(app_handle))?;

    Ok(())
}

fn maintain_bar_window<R: Runtime>(app: AppHandle<R>) {
    loop {
        let bar_window = if let Some(bar_window) = app.get_webview_window(BAR_WINDOW_LABEL) {
            bar_window
        } else {
            let Some(config) = app
                .config()
                .app
                .windows
                .iter()
                .find(|config| config.label == BAR_WINDOW_LABEL)
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
        run_taskbar_sync_loop(window_handle.0 as isize);
        wait_before_recovery();
    }
}

fn wait_before_recovery() {
    thread::sleep(RECOVERY_RETRY_DELAY);
}

fn run_taskbar_sync_loop(window_handle: isize) {
    let foreground_hook = install_win_event_hook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND);
    let structure_hook = install_win_event_hook(EVENT_OBJECT_CREATE, EVENT_OBJECT_REORDER);
    let location_hook =
        install_win_event_hook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE);
    let hook_fallback_needed =
        foreground_hook.is_none() || structure_hook.is_none() || location_hook.is_none();

    let bar = HWND(window_handle as *mut _);
    let mut current_owner = HWND::default();
    let mut last_bar_rect = None;
    let mut placement_needs_update = true;

    loop {
        // SAFETY: `bar` 仅作为不透明窗口句柄使用，并在每轮同步前验证有效性。
        if !unsafe { IsWindow(Some(bar)).as_bool() } {
            break;
        }

        let mut retry_needed = false;
        let taskbar = find_primary_taskbar();
        let Some(taskbar) = taskbar else {
            current_owner = HWND::default();
            last_bar_rect = None;
            hide_bar(bar);
            let _ = wait_for_taskbar_change(RECOVERY_RETRY_DELAY);
            continue;
        };

        let Some(taskbar_rect) = window_rect(taskbar) else {
            current_owner = HWND::default();
            last_bar_rect = None;
            hide_bar(bar);
            let _ = wait_for_taskbar_change(RECOVERY_RETRY_DELAY);
            continue;
        };
        if current_owner != taskbar {
            attach_bar_to_taskbar(bar, taskbar);
            if is_bar_attached_to_taskbar(bar, taskbar) {
                current_owner = taskbar;
            } else {
                retry_needed = true;
            }
            last_bar_rect = None;
            placement_needs_update = true;
        }

        let auto_hide_enabled = taskbar_auto_hide_enabled();
        let auto_hide_transitioning = auto_hide_enabled
            && !monitor_rect(taskbar).is_some_and(|monitor| is_rect_within(taskbar_rect, monitor));
        let hidden_for_fullscreen = !auto_hide_enabled
            && is_taskbar_covered_by_fullscreen_window(bar, taskbar, taskbar_rect);

        let taskbar_hidden = !unsafe { IsWindowVisible(taskbar).as_bool() }
            || taskbar_rect.width() <= 0
            || taskbar_rect.height() <= 2;

        if hidden_for_fullscreen || taskbar_hidden || auto_hide_transitioning {
            hide_bar(bar);
        } else {
            let mut placement_changed = false;
            if placement_needs_update || last_bar_rect.is_none() {
                let tray_left = find_system_tray_left_edge(taskbar, taskbar_rect);
                if tray_left.is_some() || last_bar_rect.is_none() {
                    // SAFETY: 任务栏句柄已验证有效，读取 DPI 不会改变窗口状态。
                    let dpi = unsafe { GetDpiForWindow(taskbar) };
                    let anchor_right = tray_left.unwrap_or(taskbar_rect.right);
                    let bar_rect = calculate_bar_rect(taskbar_rect, anchor_right, dpi);
                    placement_changed = last_bar_rect != Some(bar_rect);
                    if !placement_changed || place_bar(bar, bar_rect) {
                        last_bar_rect = Some(bar_rect);
                        placement_needs_update = false;
                    } else {
                        last_bar_rect = None;
                        retry_needed = true;
                    }
                }
            }

            if last_bar_rect.is_some()
                && !placement_changed
                && !unsafe { IsWindowVisible(bar).as_bool() }
            {
                show_bar(bar);
            }
        }

        let is_attached = is_bar_attached_to_taskbar(bar, taskbar)
            && extended_window_style(bar).contains(WS_EX_TOPMOST);
        if !is_attached {
            current_owner = HWND::default();
            retry_needed = true;
        }

        let wait_timeout = if retry_needed || hook_fallback_needed {
            RECOVERY_RETRY_DELAY
        } else {
            WINDOW_HEALTH_CHECK_INTERVAL
        };
        match wait_for_taskbar_change(wait_timeout) {
            TaskbarChange::Layout => placement_needs_update = true,
            TaskbarChange::Timeout if hook_fallback_needed => placement_needs_update = true,
            TaskbarChange::WindowState | TaskbarChange::Timeout => {}
        }
    }

    uninstall_win_event_hook(foreground_hook);
    uninstall_win_event_hook(structure_hook);
    uninstall_win_event_hook(location_hook);
}

fn install_win_event_hook(event_min: u32, event_max: u32) -> Option<HWINEVENTHOOK> {
    // SAFETY: 回调是静态函数，进程外投递模式仍在本进程执行；当前监控线程在钩子存续期间持有 Win32 消息循环。
    let hook = unsafe {
        SetWinEventHook(
            event_min,
            event_max,
            None,
            Some(handle_win_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    (!hook.is_invalid()).then_some(hook)
}

fn uninstall_win_event_hook(hook: Option<HWINEVENTHOOK>) {
    if let Some(hook) = hook {
        // SAFETY: 钩子由当前监控线程创建，并且只在这里释放一次。
        let _ = unsafe { UnhookWinEvent(hook) };
    }
}

unsafe extern "system" fn handle_win_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    object_id: i32,
    _child_id: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND {
        WINDOW_STATE_CHANGED.store(true, Ordering::Relaxed);
        return;
    }
    if window.0.is_null() || (event == EVENT_OBJECT_LOCATIONCHANGE && object_id != OBJID_WINDOW.0) {
        return;
    }

    // SAFETY: 此调用只读取 WinEvent 回调提供的借用句柄。
    let root = unsafe { GetAncestor(window, GA_ROOT) };
    let taskbar = find_primary_taskbar();
    let is_taskbar_event = taskbar.is_some_and(|taskbar| root == taskbar);
    let taskbar_location_changed =
        is_taskbar_event && event == EVENT_OBJECT_LOCATIONCHANGE && object_id == OBJID_WINDOW.0;
    let taskbar_structure_changed = is_taskbar_event
        && matches!(
            event,
            EVENT_OBJECT_CREATE
                | EVENT_OBJECT_DESTROY
                | EVENT_OBJECT_SHOW
                | EVENT_OBJECT_HIDE
                | EVENT_OBJECT_REORDER
        );
    // SAFETY: 返回的前台窗口句柄仅按值比较，不接管其所有权。
    let foreground_geometry_changed = event == EVENT_OBJECT_LOCATIONCHANGE
        && window == root
        && root == unsafe { GetForegroundWindow() };

    if foreground_geometry_changed || taskbar_location_changed || taskbar_structure_changed {
        WINDOW_STATE_CHANGED.store(true, Ordering::Relaxed);
    }
    if taskbar_location_changed || taskbar_structure_changed {
        TASKBAR_LAYOUT_CHANGED.store(true, Ordering::Relaxed);
    }
}

fn wait_for_taskbar_change(timeout: Duration) -> TaskbarChange {
    // 进程外 WinEvent 会投递到安装钩子的线程。消息感知等待可立即响应事件；超时仅用于
    // 检查窗口是否存活，以及在 API 或钩子失效后加快恢复，不参与 bar 动画。
    let deadline = Instant::now() + timeout;
    loop {
        if WINDOW_STATE_CHANGED.swap(false, Ordering::Relaxed) {
            return take_pending_taskbar_change();
        }

        let wait_millis = deadline
            .saturating_duration_since(Instant::now())
            .as_millis() as u32;
        let _ = unsafe {
            MsgWaitForMultipleObjectsEx(None, wait_millis, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
        };

        let mut message = MSG::default();
        // SAFETY: `message` 在每次调用期间均为有效存储，消息也在所属线程分发。
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() } {
            let _ = unsafe { TranslateMessage(&message) };
            unsafe { DispatchMessageW(&message) };
        }

        if WINDOW_STATE_CHANGED.swap(false, Ordering::Relaxed) {
            return take_pending_taskbar_change();
        }
        if Instant::now() >= deadline {
            return TaskbarChange::Timeout;
        }
    }
}

fn take_pending_taskbar_change() -> TaskbarChange {
    if TASKBAR_LAYOUT_CHANGED.swap(false, Ordering::Relaxed) {
        TaskbarChange::Layout
    } else {
        TaskbarChange::WindowState
    }
}

fn taskbar_auto_hide_enabled() -> bool {
    let mut data = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        ..Default::default()
    };
    // SAFETY: `data` 已填写 Win32 要求的结构体大小，并在调用期间保持有效。
    unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) as u32 & ABS_AUTOHIDE != 0 }
}

fn monitor_rect(window: HWND) -> Option<ScreenRect> {
    // SAFETY: `window` 是借用的有效句柄；使用最近显示器可避免窗口越过屏幕边缘时返回空值。
    let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_invalid() {
        return None;
    }

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` 已填写所需大小，并为此调用提供有效的可写存储。
    unsafe { GetMonitorInfoW(monitor, &mut info).as_bool() }.then(|| info.rcMonitor.into())
}

fn is_rect_within(rect: ScreenRect, bounds: ScreenRect) -> bool {
    rect.left >= bounds.left
        && rect.top >= bounds.top
        && rect.right <= bounds.right
        && rect.bottom <= bounds.bottom
}

fn find_primary_taskbar() -> Option<HWND> {
    // SAFETY: 类名是有效且以空字符结尾的 UTF-16 字符串。
    unsafe { FindWindowW(PCWSTR(windows::core::w!("Shell_TrayWnd").as_ptr()), None).ok() }
}

fn find_system_tray_left_edge(taskbar: HWND, taskbar_rect: ScreenRect) -> Option<i32> {
    // SAFETY: 调用方已验证 `taskbar`，类名也是以空字符结尾的有效字符串。
    let tray = unsafe {
        FindWindowExW(
            Some(taskbar),
            None,
            PCWSTR(windows::core::w!("TrayNotifyWnd").as_ptr()),
            None,
        )
        .ok()
    }?;
    let tray_rect = window_rect(tray)?;
    (tray_rect.left > taskbar_rect.left
        && tray_rect.left <= taskbar_rect.right
        && tray_rect.right <= taskbar_rect.right
        && tray_rect.top < taskbar_rect.bottom
        && tray_rect.bottom > taskbar_rect.top)
        .then_some(tray_rect.left)
}

fn calculate_bar_rect(taskbar: ScreenRect, anchor_right: i32, dpi: u32) -> ScreenRect {
    let dpi = if dpi == 0 { BASE_DPI } else { dpi };
    let width = scale_dip(BAR_WIDTH_DIP, dpi);

    ScreenRect {
        left: anchor_right - width,
        top: taskbar.top,
        right: anchor_right,
        bottom: taskbar.bottom,
    }
}

fn scale_dip(value: i32, dpi: u32) -> i32 {
    ((i64::from(value) * i64::from(dpi) + i64::from(BASE_DPI / 2)) / i64::from(BASE_DPI)) as i32
}

fn is_taskbar_covered_by_fullscreen_window(
    bar: HWND,
    taskbar_window: HWND,
    taskbar: ScreenRect,
) -> bool {
    // SAFETY: 返回的前台窗口句柄是借用值，并会在使用前检查。
    let foreground = unsafe { GetForegroundWindow() };
    if foreground == bar || foreground.0.is_null() {
        return false;
    }

    // 点击任务栏空白处会让 Shell_TrayWnd 或其子窗口成为前台窗口。它的矩形天然覆盖
    // 任务栏，但并不代表应用进入全屏。
    // SAFETY: GetAncestor 返回前台窗口的借用根句柄。
    let foreground_root = unsafe { GetAncestor(foreground, GA_ROOT) };
    if foreground_root == taskbar_window || is_shell_surface(foreground_root) {
        return false;
    }

    let Some(rect) = window_rect(foreground_root) else {
        return false;
    };

    rect.left <= taskbar.left
        && rect.right >= taskbar.right
        && rect.top <= taskbar.top
        && rect.bottom >= taskbar.bottom
}

fn is_shell_surface(window: HWND) -> bool {
    matches!(
        window_class_name(window).as_deref(),
        Some("Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
    )
}

fn window_class_name(window: HWND) -> Option<String> {
    let mut buffer = [0_u16; 256];
    // SAFETY: `buffer` 的完整长度均可写，`window` 是借用的有效句柄。
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    if length <= 0 {
        return None;
    }

    String::from_utf16(&buffer[..length as usize]).ok()
}

fn attach_bar_to_taskbar(bar: HWND, taskbar: HWND) {
    let mut ex_style = extended_window_style(bar);
    ex_style |= WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    ex_style &= !WS_EX_APPWINDOW;

    // SAFETY: 两个句柄均属于有效的顶层窗口。对顶层窗口设置 GWLP_HWNDPARENT 建立的是
    // 所有者关系，而非父子窗口关系；修改时保留其他扩展样式位。
    unsafe {
        SetWindowLongPtrW(bar, GWL_EXSTYLE, ex_style.0 as isize);
        SetWindowLongPtrW(bar, GWLP_HWNDPARENT, taskbar.0 as isize);
    }
}

fn is_bar_attached_to_taskbar(bar: HWND, taskbar: HWND) -> bool {
    let ex_style = extended_window_style(bar);
    // SAFETY: 从已验证窗口读取所有者句柄不会转移句柄所有权。
    let owner_matches = unsafe { GetWindowLongPtrW(bar, GWLP_HWNDPARENT) } == taskbar.0 as isize;

    owner_matches
        && ex_style.contains(WS_EX_TOOLWINDOW)
        && ex_style.contains(WS_EX_NOACTIVATE)
        && !ex_style.contains(WS_EX_APPWINDOW)
}

fn place_bar(bar: HWND, rect: ScreenRect) -> bool {
    // SAFETY: 同步循环已验证 `bar`，坐标使用 Win32 所需的物理像素。
    unsafe {
        SetWindowPos(
            bar,
            Some(HWND_TOPMOST),
            rect.left,
            rect.top,
            rect.width(),
            rect.height(),
            SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
        )
        .is_ok()
    }
}

fn show_bar(bar: HWND) {
    // 恢复置顶顺序，但保留上一次确认有效的位置和尺寸。
    let _ = unsafe {
        SetWindowPos(
            bar,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
        )
    };
}

fn hide_bar(bar: HWND) {
    // SAFETY: 两个调用只读取或隐藏已经验证的 bar 窗口句柄。
    if unsafe { IsWindowVisible(bar).as_bool() } {
        let _ = unsafe { ShowWindow(bar, SW_HIDE) };
    }
}

fn window_rect(window: HWND) -> Option<ScreenRect> {
    let mut rect = RECT::default();
    // SAFETY: `rect` 是有效的可写存储，`window` 仅作为借用句柄使用。
    unsafe { GetWindowRect(window, &mut rect).ok()? };
    Some(rect.into())
}

fn extended_window_style(window: HWND) -> WINDOW_EX_STYLE {
    // SAFETY: 从借用句柄读取窗口样式不会改变窗口状态。
    WINDOW_EX_STYLE(unsafe { GetWindowLongPtrW(window, GWL_EXSTYLE) } as u32)
}
