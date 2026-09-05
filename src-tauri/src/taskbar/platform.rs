//! 封装任务栏所需的 Win32 窗口与 Shell 操作。

use windows::{
    Win32::{
        Foundation::HWND,
        System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
        UI::{
            HiDpi::GetDpiForWindow,
            Shell::{ABM_GETSTATE, ABS_AUTOHIDE, APPBARDATA, SHAppBarMessage},
            WindowsAndMessaging::{
                FindWindowExW, FindWindowW, GA_ROOT, GWL_EXSTYLE, GWLP_HWNDPARENT, GetAncestor,
                GetClassNameW, GetForegroundWindow, GetWindowLongPtrW, HWND_TOPMOST, IsWindow,
                IsWindowVisible, SW_HIDE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER,
                SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, ShowWindow,
                WINDOW_EX_STYLE, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
                WS_EX_TOPMOST,
            },
        },
    },
    core::PCWSTR,
};

use super::geometry::{ScreenRect, window_rect};

/// 读取 Windows 11 任务栏按钮对齐方式；读取失败时使用系统默认的居中布局。
pub(super) fn taskbar_buttons_center_aligned() -> bool {
    let mut alignment = 1_u32;
    let mut byte_count = std::mem::size_of_val(&alignment) as u32;
    // SAFETY: 使用预定义的当前用户根键，只读取一个 REG_DWORD 到有效的可写存储。
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            windows::core::w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced"),
            windows::core::w!("TaskbarAl"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut alignment).cast()),
            Some(&mut byte_count),
        )
    };

    if result.is_err() {
        log::warn!("读取 Windows 任务栏对齐方式失败，按居中布局处理: {result:?}");
    }

    alignment != 0
}

/// 查找主任务栏窗口。
pub(super) fn find_primary_taskbar() -> Option<HWND> {
    // SAFETY: 类名是有效且以空字符结尾的 UTF-16 字符串。
    unsafe { FindWindowW(PCWSTR(windows::core::w!("Shell_TrayWnd").as_ptr()), None).ok() }
}

/// 查找系统托盘左边界，用于避免播放器覆盖托盘图标。
pub(super) fn find_system_tray_left_edge(taskbar: HWND, taskbar_rect: ScreenRect) -> Option<i32> {
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

/// 查询系统任务栏是否启用了自动隐藏。
pub(super) fn taskbar_auto_hide_enabled() -> bool {
    let mut data = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        ..Default::default()
    };
    // SAFETY: `data` 已填写 Win32 要求的结构体大小，并在调用期间保持有效。
    unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) as u32 & ABS_AUTOHIDE != 0 }
}

/// 判断任务栏是否被当前前台全屏窗口覆盖。
pub(super) fn is_taskbar_covered_by_fullscreen_window(
    bar: HWND,
    taskbar_window: HWND,
    taskbar: ScreenRect,
) -> bool {
    // SAFETY: 返回的前台窗口句柄是借用值，并会在使用前检查。
    let foreground = unsafe { GetForegroundWindow() };
    if foreground == bar || foreground.0.is_null() {
        return false;
    }

    // 点击任务栏空白处会让 Shell_TrayWnd 或其子窗口成为前台窗口，但不代表应用进入全屏。
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

/// 将播放器窗口设置为任务栏的所有者窗口，并调整扩展样式。
pub(super) fn attach_bar_to_taskbar(bar: HWND, taskbar: HWND) {
    let mut ex_style = extended_window_style(bar);
    ex_style |= WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    ex_style &= !WS_EX_APPWINDOW;

    // SAFETY: 两个句柄均属于有效的顶层窗口。设置 GWLP_HWNDPARENT 建立所有者关系，
    // 而非父子窗口关系；修改时保留其他扩展样式位。
    unsafe {
        SetWindowLongPtrW(bar, GWL_EXSTYLE, ex_style.0 as isize);
        SetWindowLongPtrW(bar, GWLP_HWNDPARENT, taskbar.0 as isize);
    }
}

/// 验证播放器窗口的所有者关系和关键扩展样式。
pub(super) fn is_bar_attached_to_taskbar(bar: HWND, taskbar: HWND) -> bool {
    let ex_style = extended_window_style(bar);
    // SAFETY: 从已验证窗口读取所有者句柄不会转移句柄所有权。
    let owner_matches = unsafe { GetWindowLongPtrW(bar, GWLP_HWNDPARENT) } == taskbar.0 as isize;

    owner_matches
        && ex_style.contains(WS_EX_TOOLWINDOW)
        && ex_style.contains(WS_EX_NOACTIVATE)
        && !ex_style.contains(WS_EX_APPWINDOW)
}

/// 判断播放器窗口是否仍处于置顶状态。
pub(super) fn is_bar_topmost(bar: HWND) -> bool {
    extended_window_style(bar).contains(WS_EX_TOPMOST)
}

/// 将播放器移动到指定矩形并显示。
pub(super) fn place_bar(bar: HWND, rect: ScreenRect) -> bool {
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

/// 恢复播放器的置顶顺序，并保留已确认的位置与尺寸。
pub(super) fn show_bar(bar: HWND) {
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

/// 在任务栏暂不可用或被全屏覆盖时隐藏播放器。
pub(super) fn hide_bar(bar: HWND) {
    if is_window_visible(bar) {
        // SAFETY: `bar` 已在同步循环中验证，只请求隐藏窗口。
        let _ = unsafe { ShowWindow(bar, SW_HIDE) };
    }
}

/// 判断窗口句柄当前是否仍有效。
pub(super) fn is_window_alive(window: HWND) -> bool {
    // SAFETY: 句柄仅用于只读有效性检查。
    unsafe { IsWindow(Some(window)).as_bool() }
}

/// 判断窗口当前是否可见。
pub(super) fn is_window_visible(window: HWND) -> bool {
    // SAFETY: 句柄仅用于只读可见性检查。
    unsafe { IsWindowVisible(window).as_bool() }
}

/// 读取窗口 DPI；零值由几何模块回退到基础 DPI。
pub(super) fn window_dpi(window: HWND) -> u32 {
    // SAFETY: 调用只读取借用窗口句柄的 DPI。
    unsafe { GetDpiForWindow(window) }
}

/// 判断窗口是否为不应计作全屏应用的 Shell 表面。
fn is_shell_surface(window: HWND) -> bool {
    matches!(
        window_class_name(window).as_deref(),
        Some("Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
    )
}

/// 读取窗口类名。
fn window_class_name(window: HWND) -> Option<String> {
    let mut buffer = [0_u16; 256];
    // SAFETY: `buffer` 的完整长度均可写，`window` 是借用的有效句柄。
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    if length <= 0 {
        return None;
    }

    String::from_utf16(&buffer[..length as usize]).ok()
}

/// 读取窗口扩展样式。
fn extended_window_style(window: HWND) -> WINDOW_EX_STYLE {
    // SAFETY: 从借用句柄读取窗口样式不会改变窗口状态。
    WINDOW_EX_STYLE(unsafe { GetWindowLongPtrW(window, GWL_EXSTYLE) } as u32)
}
