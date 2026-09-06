//! 封装任务栏所需的 Win32 窗口与 Shell 操作。

use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            CreateRectRgn, DeleteObject, HGDIOBJ, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME,
            RDW_INVALIDATE, RDW_UPDATENOW, RedrawWindow, SetWindowRgn,
        },
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

/// 查找系统托盘矩形，用于统一计算播放器锚点并排除托盘内的可访问性按钮。
pub(super) fn find_system_tray_rect(taskbar: HWND, taskbar_rect: ScreenRect) -> Option<ScreenRect> {
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
        .then_some(tray_rect)
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

/// 将播放器窗口设置为任务栏的拥有窗口，并调整扩展样式。
pub(super) fn attach_bar_to_taskbar(bar: HWND, taskbar: HWND) {
    let mut ex_style = extended_window_style(bar);
    ex_style |= WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    ex_style &= !WS_EX_APPWINDOW;

    // SAFETY: 两个句柄均属于有效的顶层窗口。GWLP_HWNDPARENT 在这里设置 owner，
    // 不会把播放器跨进程改成任务栏的子窗口；扩展样式修改时保留其他位。
    unsafe {
        SetWindowLongPtrW(bar, GWL_EXSTYLE, ex_style.0 as isize);
        SetWindowLongPtrW(bar, GWLP_HWNDPARENT, taskbar.0 as isize);
    }
}

/// 验证播放器窗口的 owner 和关键扩展样式。
pub(super) fn is_bar_attached_to_taskbar(bar: HWND, taskbar: HWND) -> bool {
    let ex_style = extended_window_style(bar);
    // SAFETY: 从已验证窗口读取 owner 句柄不会转移句柄所有权。
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

/// 将播放器移动到指定矩形；窗口显示状态由同步循环单独管理。
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
            SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
        .is_ok()
    }
}

/// 在移动已裁剪窗口前撤销旧区域，避免 WebView 子表面沿用旧的可绘制范围。
pub(super) fn clear_bar_clip_before_move(bar: HWND) -> bool {
    // SAFETY: 空 region 恢复完整窗口区域；此处不立即重绘，新的位置和 region 会紧接着提交。
    (unsafe { SetWindowRgn(bar, None, false) }) != 0
}

/// 将屏幕坐标下的可见矩形转换为窗口局部区域，窗口本身与 WebView 始终保留完整尺寸。
pub(super) fn clip_bar(bar: HWND, window_rect: ScreenRect, visible_rect: ScreenRect) -> bool {
    let width = window_rect.width().max(0);
    let height = window_rect.height().max(0);
    let left = (visible_rect.left - window_rect.left).clamp(0, width);
    let right = (visible_rect.right - window_rect.left).clamp(left, width);

    if left == 0 && right == width {
        // SAFETY: 传入空区域会移除窗口原有 region，恢复完整窗口绘制范围。
        return unsafe { SetWindowRgn(bar, None, true) } != 0;
    }

    // SAFETY: 坐标已经限制在窗口客户区内；成功后 region 所有权转移给系统。
    let region = unsafe { CreateRectRgn(left, 0, right, height) };
    if region.is_invalid() {
        return false;
    }

    // SAFETY: `bar` 是同步循环验证过的窗口；失败时所有权仍属于调用方并在下方释放。
    if unsafe { SetWindowRgn(bar, Some(region), true) } != 0 {
        true
    } else {
        // SAFETY: SetWindowRgn 失败，region 所有权未转移，必须由调用方释放。
        let _ = unsafe { DeleteObject(HGDIOBJ(region.0)) };
        false
    }
}

/// 使窗口及其 WebView 子窗口立即重绘新暴露的区域。
pub(super) fn redraw_bar(bar: HWND) {
    // SAFETY: `bar` 是借用的有效窗口句柄；不传矩形和 region 表示刷新整个窗口树。
    let _ = unsafe {
        RedrawWindow(
            Some(bar),
            None,
            None,
            RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
        )
    };
}

/// 恢复播放器的任务栏层级，并保留已确认的位置与尺寸。
pub(super) fn show_bar(bar: HWND) {
    // SAFETY: `bar` 是借用的有效窗口句柄，调用不激活窗口也不改动 owner 顺序。
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
