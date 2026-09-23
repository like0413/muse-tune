//! 使用 Win32 所有者窗口关系，将 Tauri bar 窗口集成到 Windows 任务栏。

mod displays;
mod elements;
mod events;
mod geometry;
mod layout;
mod platform;
mod service;
mod settings;
mod sync;
mod volume_popup;

use std::time::Duration;
use tauri::{Runtime, WebviewWindow};

use crate::error::Error;

pub use displays::TaskbarDisplay;
pub use geometry::TaskbarPlacement;
pub(crate) use service::TaskbarService;

const TASKBAR_WINDOW_LABEL: &str = "taskbar";
const RECOVERY_RETRY_DELAY: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[repr(u8)]
#[serde(rename_all = "lowercase")]
pub enum TaskbarOverlapPriority {
    #[default]
    Bar,
    #[serde(rename = "taskbar")]
    TaskbarElements,
}

impl TaskbarOverlapPriority {
    /// 从跨线程存储值恢复遮挡优先级，非法值回退到播放器优先。
    const fn from_stored(value: u8) -> Self {
        if value == Self::TaskbarElements as u8 {
            Self::TaskbarElements
        } else {
            Self::Bar
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[repr(u8)]
#[serde(rename_all = "lowercase")]
pub enum TaskbarWidthMode {
    /// 使用设置里保存的固定宽度。
    #[default]
    Fixed,
    /// 自动占用停靠侧到最近任务栏元素之间的空白，不低于最小宽度。
    Auto,
}

impl TaskbarWidthMode {
    /// 从跨线程存储值恢复宽度模式，非法值回退到固定宽度。
    const fn from_stored(value: u8) -> Self {
        if value == Self::Auto as u8 {
            Self::Auto
        } else {
            Self::Fixed
        }
    }
}

pub fn set_placement(placement: TaskbarPlacement) {
    settings::set_placement(placement);
}

pub fn set_overlap_priority(priority: TaskbarOverlapPriority) {
    settings::set_overlap_priority(priority);
}

pub fn set_content_width(width: i32) {
    settings::set_content_width(width);
}

pub fn set_width_mode(mode: TaskbarWidthMode) {
    settings::set_width_mode(mode);
}

pub fn set_content_visibility(visible: bool) {
    settings::set_content_visibility(visible);
}

pub(crate) fn content_visible() -> bool {
    settings::content_visible()
}

/// 返回当前拥有 Windows 任务栏的显示器。
pub fn available_displays() -> Vec<TaskbarDisplay> {
    displays::available_taskbar_displays()
}

/// 返回诊断页所需的目标显示器和当前原生布局设置。
pub(crate) fn diagnostic_settings() -> (
    String,
    TaskbarPlacement,
    TaskbarOverlapPriority,
    TaskbarWidthMode,
    i32,
) {
    let target = service::display_target_snapshot().0;
    let (placement, overlap_priority, width_mode, width) = settings::diagnostic_settings();
    (target, placement, overlap_priority, width_mode, width)
}

/// 显示并定位独立音量悬浮窗。
pub fn show_volume_popup<R: Runtime>(
    source: &WebviewWindow<R>,
    anchor_center_x: f64,
    theme_color: String,
) -> Result<(), Error> {
    volume_popup::show(source, anchor_center_x, theme_color)
}

/// 仅允许音量悬浮窗隐藏自身。
pub fn hide_volume_popup<R: Runtime>(
    source: &WebviewWindow<R>,
    generation: u64,
) -> Result<(), Error> {
    volume_popup::hide(source, generation)
}

/// 更新目标显示器，并立即唤醒窗口管理线程。
pub fn set_display_target(target: String) -> Result<(), Error> {
    service::set_display_target(target)
}

/// 接收任务栏 WinEvent，立即要求窗口管理线程重新枚举显示器。
pub(super) fn request_display_refresh() {
    service::request_display_refresh();
}

/// 启动独立监控线程，持续维护任务栏与播放器窗口的所有者关系。
pub fn initialize<R: Runtime>(
    app: &mut tauri::App<R>,
) -> Result<TaskbarService, Box<dyn std::error::Error>> {
    service::initialize(app)
}
