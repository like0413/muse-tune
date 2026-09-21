//! 任务栏布局设置的运行时快照。
//!
//! 这些值跨线程共享：设置页经 command 写入，每个 bar 的同步线程按帧读取。
//! 集中在一处并统一从原生默认值初始化，避免读取方各自持有副本，
//! 出现“设置已经改了、布局却没动”的错位。

use std::sync::{
    LazyLock,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
};

use crate::native_defaults;

use super::{TaskbarOverlapPriority, TaskbarWidthMode, events, geometry::TaskbarPlacement};

// 下列静态值的初值即共享默认值（见 `native-defaults.json`）。`restore_native_settings` 会在
// 创建任务栏窗口之前用已持久化的设置覆盖它们，因此默认值只在共享文件中定义一次。
static TASKBAR_PLACEMENT: LazyLock<AtomicU8> =
    LazyLock::new(|| AtomicU8::new(native_defaults::shared().taskbar.placement as u8));
static TASKBAR_OVERLAP_PRIORITY: LazyLock<AtomicU8> =
    LazyLock::new(|| AtomicU8::new(native_defaults::shared().taskbar.overlap_priority as u8));
static TASKBAR_CONTENT_WIDTH_DIP: LazyLock<AtomicI32> =
    LazyLock::new(|| AtomicI32::new(native_defaults::shared().taskbar.width));
static TASKBAR_WIDTH_MODE: LazyLock<AtomicU8> =
    LazyLock::new(|| AtomicU8::new(native_defaults::shared().taskbar.width_mode as u8));
static TASKBAR_CONTENT_VISIBLE: AtomicBool = AtomicBool::new(true);

/// bar 基准宽度的可调下限，与前端共用同一份取值。
pub(super) fn min_content_width() -> i32 {
    native_defaults::shared().taskbar.width_min
}

/// bar 基准宽度的可调上限，与前端共用同一份取值。
pub(super) fn max_content_width() -> i32 {
    native_defaults::shared().taskbar.width_max
}

/// 跨线程更新定位偏好；仅在值变化时唤醒同步循环。
pub(super) fn set_placement(placement: TaskbarPlacement) {
    if TASKBAR_PLACEMENT.swap(placement as u8, Ordering::AcqRel) != placement as u8 {
        events::request_all_layout_updates();
    }
}

/// 跨线程更新元素遮挡优先级；仅在值变化时唤醒同步循环。
pub(super) fn set_overlap_priority(priority: TaskbarOverlapPriority) {
    if TASKBAR_OVERLAP_PRIORITY.swap(priority as u8, Ordering::AcqRel) != priority as u8 {
        events::request_all_layout_updates();
    }
}

/// 跨线程更新 bar 基准宽度；仅在值变化时唤醒全部同步线程。
pub(super) fn set_content_width(width: i32) {
    let width = width.clamp(min_content_width(), max_content_width());
    if TASKBAR_CONTENT_WIDTH_DIP.swap(width, Ordering::AcqRel) != width {
        events::request_all_layout_updates();
    }
}

/// 跨线程更新宽度模式；仅在值变化时唤醒全部同步线程。
pub(super) fn set_width_mode(mode: TaskbarWidthMode) {
    if TASKBAR_WIDTH_MODE.swap(mode as u8, Ordering::AcqRel) != mode as u8 {
        events::request_all_layout_updates();
    }
}

/// 更新 bar 内容可见性；值变化时立即唤醒全部同步线程。
pub(super) fn set_content_visibility(visible: bool) {
    if TASKBAR_CONTENT_VISIBLE.swap(visible, Ordering::AcqRel) != visible {
        events::request_all_layout_updates();
    }
}

pub(super) fn placement() -> TaskbarPlacement {
    TaskbarPlacement::from_stored(TASKBAR_PLACEMENT.load(Ordering::Acquire))
}

pub(super) fn overlap_priority() -> TaskbarOverlapPriority {
    TaskbarOverlapPriority::from_stored(TASKBAR_OVERLAP_PRIORITY.load(Ordering::Acquire))
}

/// 读取已经规范化的 bar 基准宽度。
pub(super) fn content_width() -> i32 {
    TASKBAR_CONTENT_WIDTH_DIP.load(Ordering::Acquire)
}

pub(super) fn width_mode() -> TaskbarWidthMode {
    TaskbarWidthMode::from_stored(TASKBAR_WIDTH_MODE.load(Ordering::Acquire))
}

/// 读取媒体状态计算出的 bar 内容可见性。
pub(super) fn content_visible() -> bool {
    TASKBAR_CONTENT_VISIBLE.load(Ordering::Acquire)
}

/// 判断是否需要任务栏元素矩形：避让任务栏元素要用于裁剪，自适应宽度要用于计算空白。
pub(super) const fn needs_taskbar_elements(
    priority: TaskbarOverlapPriority,
    mode: TaskbarWidthMode,
) -> bool {
    matches!(priority, TaskbarOverlapPriority::TaskbarElements)
        || matches!(mode, TaskbarWidthMode::Auto)
}

/// 返回当前生效的定位、遮挡优先级与逻辑宽度。
pub(super) fn diagnostic_settings() -> (
    TaskbarPlacement,
    TaskbarOverlapPriority,
    TaskbarWidthMode,
    i32,
) {
    (
        placement(),
        overlap_priority(),
        width_mode(),
        content_width(),
    )
}
