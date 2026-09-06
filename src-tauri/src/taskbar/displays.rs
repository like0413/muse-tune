//! 枚举 Windows 为各显示器创建的任务栏窗口。

use std::collections::HashSet;

use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONULL, MONITORINFO, MONITORINFOEXW, MonitorFromWindow,
        },
        UI::WindowsAndMessaging::{FindWindowExW, FindWindowW, MONITORINFOF_PRIMARY},
    },
    core::PCWSTR,
};

use super::geometry::ScreenRect;

/// 提供给设置页的任务栏显示器信息。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarDisplay {
    pub id: String,
    pub label: String,
    pub width: i32,
    pub height: i32,
    pub is_primary: bool,
}

/// 原生同步线程所需的显示器与任务栏句柄。
pub(super) struct TaskbarDisplayHandle {
    pub(super) display: TaskbarDisplay,
    pub(super) taskbar: isize,
    rect: ScreenRect,
}

/// 枚举当前桌面上由 Explorer 创建的主任务栏和副任务栏。
pub(super) fn taskbar_displays() -> Vec<TaskbarDisplayHandle> {
    let mut taskbars = Vec::new();
    // SAFETY: 类名是静态空字符结尾字符串，仅查找顶层窗口。
    if let Ok(primary) = unsafe { FindWindowW(windows::core::w!("Shell_TrayWnd"), None) } {
        taskbars.push(primary);
    }

    let mut previous = None;
    loop {
        // SAFETY: 类名为静态空字符结尾字符串；previous 仅引用上次枚举得到的顶层窗口。
        let next = unsafe {
            FindWindowExW(
                None,
                previous,
                PCWSTR(windows::core::w!("Shell_SecondaryTrayWnd").as_ptr()),
                None,
            )
        };
        let Ok(taskbar) = next else {
            break;
        };
        taskbars.push(taskbar);
        previous = Some(taskbar);
    }

    let mut displays: Vec<_> = taskbars
        .into_iter()
        .filter_map(display_for_taskbar)
        .collect();
    displays.sort_by_key(|display| {
        (
            !display.display.is_primary,
            display.rect.top,
            display.rect.left,
        )
    });
    let mut seen_ids = HashSet::with_capacity(displays.len());
    displays.retain(|display| seen_ids.insert(display.display.id.clone()));

    for (index, display) in displays.iter_mut().enumerate() {
        display.display.label = format!("显示器 {}", index + 1);
    }
    displays
}

/// 返回设置页可选择的当前任务栏显示器。
pub fn available_taskbar_displays() -> Vec<TaskbarDisplay> {
    taskbar_displays()
        .into_iter()
        .map(|display| display.display)
        .collect()
}

/// 将任务栏窗口映射到所属显示器的逻辑设备名与边界。
fn display_for_taskbar(taskbar: HWND) -> Option<TaskbarDisplayHandle> {
    // SAFETY: taskbar 是枚举所得窗口；未映射到显示器时返回空句柄并在下方排除。
    let monitor = unsafe { MonitorFromWindow(taskbar, MONITOR_DEFAULTTONULL) };
    if monitor.is_invalid() {
        return None;
    }

    let mut info = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: info 的 cbSize 与实际结构一致，并提供完整可写存储。
    if !unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo) }.as_bool() {
        return None;
    }

    let device_length = info
        .szDevice
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(info.szDevice.len());
    let id = String::from_utf16(&info.szDevice[..device_length]).ok()?;
    let monitor_rect = ScreenRect::from(info.monitorInfo.rcMonitor);
    if id.is_empty() || monitor_rect.width() <= 0 || monitor_rect.height() <= 0 {
        return None;
    }

    Some(TaskbarDisplayHandle {
        display: TaskbarDisplay {
            id,
            label: String::new(),
            width: monitor_rect.width(),
            height: monitor_rect.height(),
            is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        },
        taskbar: taskbar.0 as isize,
        rect: monitor_rect,
    })
}
