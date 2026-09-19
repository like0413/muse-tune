//! 负责任务栏窗口的矩形计算与 DPI 缩放。

use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    UI::WindowsAndMessaging::GetWindowRect,
};

const BASE_DPI: u32 = 96;
const RIGHT_CLIP_CLEARANCE_DIP: i32 = 8;
/// 自适应宽度与最近任务栏元素之间保留的视觉间距。
const AUTO_WIDTH_ELEMENT_GAP_DIP: i32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TaskbarSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize)]
#[repr(u8)]
#[serde(rename_all = "lowercase")]
pub enum TaskbarPlacement {
    #[default]
    Auto,
    Left,
    Right,
}

impl TaskbarPlacement {
    /// 从跨线程存储值恢复定位偏好，非法值回退到自动模式。
    pub(super) const fn from_stored(value: u8) -> Self {
        match value {
            value if value == Self::Left as u8 => Self::Left,
            value if value == Self::Right as u8 => Self::Right,
            _ => Self::Auto,
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

    /// 判断两个矩形是否存在正面积交集。
    pub(super) fn intersects(self, other: Self) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.top < other.bottom
            && self.bottom > other.top
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
    content_width_dip: i32,
) -> ScreenRect {
    let dpi = if dpi == 0 { BASE_DPI } else { dpi };
    let width = scale_dip(content_width_dip.max(1), dpi).min(taskbar.width());
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

/// 计算播放器的硬裁剪区域，从停靠侧相反的一端避开实际任务栏元素。
pub(super) fn hard_clip_bar_rect(
    bar: ScreenRect,
    elements: &[ScreenRect],
    side: TaskbarSide,
    dpi: u32,
) -> ScreenRect {
    match side {
        TaskbarSide::Left => ScreenRect {
            right: elements
                .iter()
                .filter(|element| bar.intersects(**element))
                .map(|element| element.left)
                .min()
                .unwrap_or(bar.right)
                .clamp(bar.left, bar.right),
            ..bar
        },
        TaskbarSide::Right => {
            let clearance = scale_dip(RIGHT_CLIP_CLEARANCE_DIP, dpi);
            ScreenRect {
                left: elements
                    .iter()
                    .filter_map(|element| {
                        // 右侧 UIA 边界缺少左侧已有的视觉留白，将碰撞区向 bar 扩展 8 DIP。
                        let expanded_right = element.right.saturating_add(clearance);
                        bar.intersects(ScreenRect {
                            right: expanded_right,
                            ..*element
                        })
                        .then_some(expanded_right)
                    })
                    .max()
                    .unwrap_or(bar.left)
                    .clamp(bar.left, bar.right),
                ..bar
            }
        }
    }
}

/// 将设备无关像素按当前窗口 DPI 转换为物理像素。
fn scale_dip(value: i32, dpi: u32) -> i32 {
    ((i64::from(value) * i64::from(dpi) + i64::from(BASE_DPI / 2)) / i64::from(BASE_DPI)) as i32
}

/// 反方向换算：把物理像素折回设备无关像素。
fn to_dip(value: i32, dpi: u32) -> i32 {
    ((i64::from(value) * i64::from(BASE_DPI) + i64::from(dpi / 2)) / i64::from(dpi)) as i32
}

/// 计算自适应模式下的内容宽度（DIP）：停靠侧到最近任务栏元素之间的空白，
/// 已扣除 bar 与元素之间的视觉间距。空间不足时返回 0，由调用方按最小宽度兜底。
pub(super) fn auto_content_width(
    taskbar: ScreenRect,
    anchor_right: i32,
    side: TaskbarSide,
    dpi: u32,
    elements: &[ScreenRect],
) -> i32 {
    let dpi = if dpi == 0 { BASE_DPI } else { dpi };
    let gap = scale_dip(AUTO_WIDTH_ELEMENT_GAP_DIP, dpi);
    let available = match side {
        TaskbarSide::Left => match elements.iter().map(|element| element.left).min() {
            Some(boundary) => boundary - gap - taskbar.left,
            // 停靠侧没有任何元素时占满整段可用区。
            None => taskbar.width(),
        },
        TaskbarSide::Right => match elements.iter().map(|element| element.right).max() {
            Some(boundary) => anchor_right - gap - boundary,
            None => anchor_right - taskbar.left,
        },
    };

    to_dip(available.max(0), dpi)
}
