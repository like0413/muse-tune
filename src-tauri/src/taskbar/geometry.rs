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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
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
    pub(super) fn width(self) -> i32 {
        self.right - self.left
    }

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
    horizontal_offset_dip: i32,
) -> ScreenRect {
    let dpi = if dpi == 0 { BASE_DPI } else { dpi };
    let width = scale_dip(content_width_dip.max(1), dpi).min(taskbar.width());
    let horizontal_offset = scale_dip(horizontal_offset_dip, dpi);
    let (left, right) = match side {
        TaskbarSide::Left => (taskbar.left, taskbar.left.saturating_add(width)),
        TaskbarSide::Right => (anchor_right.saturating_sub(width), anchor_right),
    };

    ScreenRect {
        left: left.saturating_add(horizontal_offset),
        top: taskbar.top,
        right: right.saturating_add(horizontal_offset),
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
    let scaled = i64::from(value) * i64::from(dpi);
    let rounding = i64::from(BASE_DPI / 2) * scaled.signum();
    ((scaled + rounding) / i64::from(BASE_DPI)) as i32
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造任务栏高度上的矩形，测试只关心横向范围。
    fn rect(left: i32, right: i32) -> ScreenRect {
        ScreenRect {
            left,
            top: 1080,
            right,
            bottom: 1128,
        }
    }

    /// 右侧停靠时播放器贴着锚点向内生长。
    #[test]
    fn right_docked_bar_grows_leftwards_from_the_anchor() {
        let bar = calculate_bar_rect(rect(0, 1920), 1800, BASE_DPI, TaskbarSide::Right, 250, 0);

        assert_eq!(bar, rect(1550, 1800));
    }

    /// 宽度不得超过任务栏可用宽度，否则播放器会被推到屏幕外。
    #[test]
    fn bar_width_is_capped_by_the_taskbar() {
        let bar = calculate_bar_rect(rect(0, 1920), 1920, BASE_DPI, TaskbarSide::Right, 4000, 0);

        assert_eq!(bar, rect(0, 1920));
    }

    /// DPI 缺失时按基准 DPI 处理，不能出现除零或退化到 0 宽度。
    #[test]
    fn missing_dpi_falls_back_to_base_dpi() {
        let bar = calculate_bar_rect(rect(0, 1920), 1800, 0, TaskbarSide::Right, 250, 0);

        assert_eq!(bar.width(), 250);
    }

    /// 高 DPI 下同一 DIP 宽度要按比例放大，否则高分屏上播放器会明显偏窄。
    #[test]
    fn dip_width_scales_with_dpi() {
        let bar = calculate_bar_rect(rect(0, 1920), 1920, 120, TaskbarSide::Right, 250, 0);

        assert_eq!(bar.width(), 313);
    }

    /// 左侧停靠时从任务栏左边缘起算，与右侧锚点无关。
    #[test]
    fn left_docked_bar_starts_at_the_taskbar_edge() {
        let bar = calculate_bar_rect(rect(0, 1920), 1800, BASE_DPI, TaskbarSide::Left, 250, 0);

        assert_eq!(bar, rect(0, 250));
    }

    /// 有符号偏移只改变 X 坐标；正值在两种停靠方向下都向屏幕右侧移动。
    #[test]
    fn horizontal_offset_uses_screen_axis_direction() {
        let left = calculate_bar_rect(rect(0, 1920), 1800, BASE_DPI, TaskbarSide::Left, 250, 24);
        let right = calculate_bar_rect(rect(0, 1920), 1800, BASE_DPI, TaskbarSide::Right, 250, -24);

        assert_eq!(left, rect(24, 274));
        assert_eq!(right, rect(1526, 1776));
    }

    /// 偏移与宽度一样按显示器 DPI 缩放，保证不同缩放比例下视觉距离一致。
    #[test]
    fn horizontal_offset_scales_with_dpi() {
        let bar = calculate_bar_rect(rect(0, 1920), 1800, 120, TaskbarSide::Right, 250, 20);

        assert_eq!(bar, rect(1512, 1825));
    }

    /// 负偏移使用与正偏移对称的 DPI 舍入，不能让小负值意外退化为零。
    #[test]
    fn negative_horizontal_offset_scales_symmetrically() {
        let positive = calculate_bar_rect(rect(0, 1920), 1800, 120, TaskbarSide::Left, 250, 1);
        let negative = calculate_bar_rect(rect(0, 1920), 1800, 120, TaskbarSide::Left, 250, -1);

        assert_eq!(positive.left, 1);
        assert_eq!(negative.left, -1);
    }

    /// 硬裁剪只为真正重叠的元素让位，且右侧要额外留出视觉间距。
    #[test]
    fn hard_clip_makes_room_for_overlapping_elements() {
        let bar = rect(1500, 1800);
        let elements = [rect(1700, 1750)];

        let clipped = hard_clip_bar_rect(bar, &elements, TaskbarSide::Right, BASE_DPI);

        assert_eq!(clipped, rect(1758, 1800));
    }

    /// 元素与播放器不重叠时保持原矩形，不能凭空裁剪出空隙。
    #[test]
    fn hard_clip_keeps_the_rect_without_overlap() {
        let bar = rect(1500, 1800);
        let elements = [rect(1000, 1200)];

        assert_eq!(
            hard_clip_bar_rect(bar, &elements, TaskbarSide::Right, BASE_DPI),
            bar
        );
    }

    /// 左侧停靠时右边界让给最靠左的重叠元素。
    #[test]
    fn left_docked_clip_uses_the_nearest_element_edge() {
        let bar = rect(0, 250);
        let elements = [rect(200, 220), rect(300, 320)];

        let clipped = hard_clip_bar_rect(bar, &elements, TaskbarSide::Left, BASE_DPI);

        assert_eq!(clipped, rect(0, 200));
    }

    /// 自适应宽度取锚点到最靠拢元素之间的空白，并扣除视觉间距。
    #[test]
    fn auto_width_uses_the_gap_to_the_nearest_element() {
        let elements = [rect(1600, 1700)];

        let width =
            auto_content_width(rect(0, 1920), 1800, TaskbarSide::Right, BASE_DPI, &elements);

        assert_eq!(width, 92);
    }

    /// 空间不足时返回 0，由调用方按最小宽度兜底，不能返回负数。
    #[test]
    fn auto_width_never_goes_negative() {
        let elements = [rect(1750, 1900)];

        let width =
            auto_content_width(rect(0, 1920), 1800, TaskbarSide::Right, BASE_DPI, &elements);

        assert_eq!(width, 0);
    }

    /// 还没读到任何元素时占满整段可用区。
    #[test]
    fn auto_width_fills_the_available_area_without_elements() {
        let width = auto_content_width(rect(0, 1920), 1800, TaskbarSide::Right, BASE_DPI, &[]);

        assert_eq!(width, 1800);
    }

    /// 贴合边界不算越界，否则与显示器同尺寸的任务栏会被判定为在屏幕外。
    #[test]
    fn rect_within_accepts_touching_edges() {
        let bounds = rect(0, 1920);

        assert!(is_rect_within(bounds, bounds));
        assert!(!is_rect_within(rect(0, 1921), bounds));
    }

    /// 仅边缘接触不算相交，否则相邻元素的裁剪会互相影响。
    #[test]
    fn intersection_requires_positive_area() {
        let left = rect(0, 100);

        assert!(!left.intersects(rect(100, 200)));
        assert!(left.intersects(rect(99, 200)));
    }
}
