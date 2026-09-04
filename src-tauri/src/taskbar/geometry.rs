//! 负责任务栏窗口的矩形计算与 DPI 缩放。

use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    UI::WindowsAndMessaging::GetWindowRect,
};

const BAR_WIDTH_DIP: i32 = 360;
const BASE_DPI: u32 = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TaskbarSide {
    Left,
    Right,
}

impl TaskbarSide {
    /// 将设置值转换为任务栏停靠方向。
    pub(super) const fn from_right_aligned(right_aligned: bool) -> Self {
        if right_aligned {
            Self::Right
        } else {
            Self::Left
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ScreenRect {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) right: i32,
    pub(super) bottom: i32,
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
    /// 返回矩形宽度。
    pub(super) fn width(self) -> i32 {
        self.right - self.left
    }

    /// 返回矩形高度。
    pub(super) fn height(self) -> i32 {
        self.bottom - self.top
    }
}

/// 读取窗口在屏幕坐标系中的矩形。
pub(super) fn window_rect(window: HWND) -> Option<ScreenRect> {
    let mut rect = RECT::default();
    // SAFETY: `rect` 是有效的可写存储，`window` 仅作为借用句柄使用。
    unsafe { GetWindowRect(window, &mut rect).ok()? };
    Some(rect.into())
}

/// 获取窗口所在显示器的完整屏幕矩形。
pub(super) fn monitor_rect(window: HWND) -> Option<ScreenRect> {
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

/// 判断一个矩形是否完整位于指定边界内。
pub(super) fn is_rect_within(rect: ScreenRect, bounds: ScreenRect) -> bool {
    rect.left >= bounds.left
        && rect.top >= bounds.top
        && rect.right <= bounds.right
        && rect.bottom <= bounds.bottom
}

/// 根据任务栏矩形、右侧锚点、DPI 和停靠方向计算播放器位置。
pub(super) fn calculate_bar_rect(
    taskbar: ScreenRect,
    anchor_right: i32,
    dpi: u32,
    side: TaskbarSide,
) -> ScreenRect {
    let dpi = if dpi == 0 { BASE_DPI } else { dpi };
    let width = scale_dip(BAR_WIDTH_DIP, dpi).min(taskbar.width());
    let (left, right) = match side {
        TaskbarSide::Left => (taskbar.left, taskbar.left + width),
        TaskbarSide::Right => (anchor_right - width, anchor_right),
    };

    ScreenRect {
        left,
        top: taskbar.top,
        right,
        bottom: taskbar.bottom,
    }
}

/// 将设备无关像素按当前窗口 DPI 转换为物理像素。
fn scale_dip(value: i32, dpi: u32) -> i32 {
    ((i64::from(value) * i64::from(dpi) + i64::from(BASE_DPI / 2)) / i64::from(BASE_DPI)) as i32
}
