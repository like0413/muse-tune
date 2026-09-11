//! 协调任务栏状态、窗口所有权与播放器布局。

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};

use windows::Win32::Foundation::HWND;

use super::{
    TaskbarOverlapPriority, content_visible,
    elements::TaskbarElements,
    events::{TaskbarChange, WinEventHooks, wait_for_taskbar_change},
    geometry::{
        ScreenRect, TaskbarPlacement, TaskbarSide, calculate_bar_rect, hard_clip_bar_rect,
        is_rect_within, monitor_rect, window_rect,
    },
    layout::{BarLayout, LayoutStabilizer},
    platform::{
        attach_bar_to_taskbar, clear_bar_clip_before_move, clip_bar, find_system_tray_rect,
        hide_bar, is_bar_attached_to_taskbar, is_bar_topmost,
        is_taskbar_covered_by_fullscreen_window, is_window_alive, is_window_visible, place_bar,
        redraw_bar, shell_reports_fullscreen_activity, show_bar, taskbar_auto_hide_enabled,
        taskbar_buttons_center_aligned, window_dpi,
    },
};

const RECOVERY_RETRY_DELAY: Duration = Duration::from_millis(400);
const FULLSCREEN_STATE_CHECK_INTERVAL: Duration = Duration::from_millis(250);
const UIA_FALLBACK_QUERY_INTERVAL: Duration = Duration::from_secs(1);
const MIN_CONTENT_WIDTH_DIP: i32 = 200;
const MAX_CONTENT_WIDTH_DIP: i32 = 360;
static TASKBAR_PLACEMENT: AtomicU8 = AtomicU8::new(TaskbarPlacement::Auto as u8);
static TASKBAR_OVERLAP_PRIORITY: AtomicU8 = AtomicU8::new(TaskbarOverlapPriority::Bar as u8);
static TASKBAR_CONTENT_WIDTH_DIP: AtomicI32 = AtomicI32::new(MAX_CONTENT_WIDTH_DIP);

/// 隔离 Shell 未提供事件的全屏状态查询，状态未变化时不唤醒完整同步流程。
struct FullscreenStateMonitor {
    active: bool,
    next_check_at: Instant,
}

impl FullscreenStateMonitor {
    /// 读取初始状态，并安排下一次必要查询。
    fn new(taskbar: HWND, now: Instant) -> Self {
        Self {
            active: shell_reports_fullscreen_activity(taskbar),
            next_check_at: now + FULLSCREEN_STATE_CHECK_INTERVAL,
        }
    }

    /// 到期时只读取一次 Shell 状态，并报告状态是否发生变化。
    fn refresh_if_due(&mut self, taskbar: HWND, now: Instant) -> bool {
        if now < self.next_check_at {
            return false;
        }
        self.next_check_at = now + FULLSCREEN_STATE_CHECK_INTERVAL;
        let active = shell_reports_fullscreen_activity(taskbar);
        if active == self.active {
            return false;
        }
        self.active = active;
        true
    }
}

/// 跨线程更新定位偏好，并通过线程消息唤醒同步循环。
pub(super) fn set_placement(placement: TaskbarPlacement) {
    TASKBAR_PLACEMENT.store(placement as u8, Ordering::Release);
    super::events::request_all_layout_updates();
}

/// 跨线程更新元素遮挡优先级，并通过线程消息唤醒同步循环。
pub(super) fn set_overlap_priority(priority: TaskbarOverlapPriority) {
    TASKBAR_OVERLAP_PRIORITY.store(priority as u8, Ordering::Release);
    super::events::request_all_layout_updates();
}

/// 跨线程更新 bar 基准宽度；仅在值变化时唤醒全部同步线程。
pub(super) fn set_content_width(width: i32) {
    let width = width.clamp(MIN_CONTENT_WIDTH_DIP, MAX_CONTENT_WIDTH_DIP);
    if TASKBAR_CONTENT_WIDTH_DIP.swap(width, Ordering::AcqRel) != width {
        super::events::request_all_layout_updates();
    }
}

/// 读取已经规范化的 bar 基准宽度。
fn content_width() -> i32 {
    TASKBAR_CONTENT_WIDTH_DIP.load(Ordering::Acquire)
}

/// 读取当前遮挡优先级。
fn overlap_priority() -> TaskbarOverlapPriority {
    TaskbarOverlapPriority::from_stored(TASKBAR_OVERLAP_PRIORITY.load(Ordering::Acquire))
}

/// 返回当前生效的定位、遮挡优先级与逻辑宽度。
pub(super) fn diagnostic_settings() -> (TaskbarPlacement, TaskbarOverlapPriority, i32) {
    (
        TaskbarPlacement::from_stored(TASKBAR_PLACEMENT.load(Ordering::Acquire)),
        overlap_priority(),
        content_width(),
    )
}

/// 将用户偏好和系统任务栏对齐方式解析为实际停靠侧。
fn resolve_taskbar_side() -> TaskbarSide {
    match TaskbarPlacement::from_stored(TASKBAR_PLACEMENT.load(Ordering::Acquire)) {
        TaskbarPlacement::Auto if taskbar_buttons_center_aligned() => TaskbarSide::Left,
        TaskbarPlacement::Auto => TaskbarSide::Right,
        TaskbarPlacement::Left => TaskbarSide::Left,
        TaskbarPlacement::Right => TaskbarSide::Right,
    }
}

/// 在窗口存活期间同步任务栏所有权、可见性、位置与硬裁剪方向。
pub(super) fn run(window_handle: isize, taskbar_handle: isize, stop: Arc<AtomicBool>) {
    let taskbar = HWND(taskbar_handle as *mut _);
    let hooks = WinEventHooks::install(taskbar);
    let hook_fallback_needed = hooks.fallback_needed();
    let mut taskbar_elements: Option<TaskbarElements> = None;
    let mut stabilizer = LayoutStabilizer::new();

    let bar = HWND(window_handle as *mut _);
    let mut current_taskbar = HWND::default();
    let mut active_priority = overlap_priority();
    let mut applied_layout: Option<BarLayout> = None;
    let mut immediate_layout_needed = active_priority == TaskbarOverlapPriority::Bar;
    let mut uia_watch_needed = active_priority == TaskbarOverlapPriority::TaskbarElements;
    let mut next_uia_fallback_query = Instant::now() + UIA_FALLBACK_QUERY_INTERVAL;
    let mut bar_was_suppressed = true;
    let mut z_order_refresh_needed = true;
    let mut fullscreen_monitor = FullscreenStateMonitor::new(taskbar, Instant::now());

    if active_priority == TaskbarOverlapPriority::TaskbarElements {
        stabilizer.invalidate(Instant::now());
    }

    while !stop.load(Ordering::Acquire) && is_window_alive(bar) {
        let now = Instant::now();
        let priority = overlap_priority();
        if priority != active_priority {
            active_priority = priority;
            stabilizer.reset();
            immediate_layout_needed = priority == TaskbarOverlapPriority::Bar;
            if priority == TaskbarOverlapPriority::TaskbarElements {
                uia_watch_needed = true;
                stabilizer.invalidate(now);
            } else {
                taskbar_elements = None;
                uia_watch_needed = false;
            }
        }

        let mut retry_needed = false;
        if !is_window_alive(taskbar) {
            break;
        }

        let Some(taskbar_rect) = window_rect(taskbar) else {
            reset_for_missing_taskbar(
                bar,
                &mut current_taskbar,
                &mut applied_layout,
                &mut stabilizer,
            );
            bar_was_suppressed = true;
            let _ = wait_for_taskbar_change(RECOVERY_RETRY_DELAY);
            continue;
        };

        if current_taskbar != taskbar || !is_bar_attached_to_taskbar(bar, taskbar) {
            attach_bar_to_taskbar(bar, taskbar);
            if is_bar_attached_to_taskbar(bar, taskbar) {
                current_taskbar = taskbar;
                uia_watch_needed = active_priority == TaskbarOverlapPriority::TaskbarElements;
            } else {
                retry_needed = true;
            }
            applied_layout = None;
            stabilizer.reset();
            immediate_layout_needed = active_priority == TaskbarOverlapPriority::Bar;
            if active_priority == TaskbarOverlapPriority::TaskbarElements {
                stabilizer.invalidate(Instant::now());
            }
        }

        if uia_watch_needed && current_taskbar == taskbar {
            if taskbar_elements.is_none() {
                taskbar_elements = TaskbarElements::new();
            }
            if let Some(elements) = taskbar_elements.as_mut() {
                elements.watch_taskbar(taskbar);
            }
            uia_watch_needed = false;
        }

        let uia_fallback_needed = active_priority == TaskbarOverlapPriority::TaskbarElements
            && taskbar_elements
                .as_ref()
                .is_some_and(|elements| !elements.is_event_driven());
        if uia_fallback_needed && now >= next_uia_fallback_query {
            stabilizer.invalidate(now);
            next_uia_fallback_query = now + UIA_FALLBACK_QUERY_INTERVAL;
        }

        let auto_hide_enabled = taskbar_auto_hide_enabled();
        let auto_hide_transitioning = auto_hide_enabled
            && !monitor_rect(taskbar).is_some_and(|monitor| is_rect_within(taskbar_rect, monitor));
        let hidden_for_fullscreen = !auto_hide_enabled
            && (is_taskbar_covered_by_fullscreen_window(bar, taskbar, taskbar_rect)
                || fullscreen_monitor.active);
        let taskbar_hidden =
            !is_window_visible(taskbar) || taskbar_rect.width() <= 0 || taskbar_rect.height() <= 2;

        if !content_visible() || hidden_for_fullscreen || taskbar_hidden || auto_hide_transitioning
        {
            hide_bar(bar);
            bar_was_suppressed = true;
            z_order_refresh_needed = true;
        } else {
            let should_measure = match active_priority {
                TaskbarOverlapPriority::Bar => immediate_layout_needed || applied_layout.is_none(),
                TaskbarOverlapPriority::TaskbarElements => stabilizer.sample_due(Instant::now()),
            };

            if should_measure {
                match measure_layout(
                    taskbar,
                    taskbar_rect,
                    active_priority,
                    taskbar_elements.as_ref(),
                ) {
                    Ok(candidate) => {
                        let ready = if active_priority == TaskbarOverlapPriority::Bar {
                            Some(candidate)
                        } else {
                            stabilizer.observe(candidate, Instant::now())
                        };
                        if let Some(layout) = ready {
                            if applied_layout == Some(layout) {
                                immediate_layout_needed = false;
                            } else if apply_layout(bar, layout, applied_layout) {
                                applied_layout = Some(layout);
                                immediate_layout_needed = false;
                            } else {
                                applied_layout = None;
                                retry_needed = true;
                                if active_priority == TaskbarOverlapPriority::TaskbarElements {
                                    stabilizer.invalidate(Instant::now());
                                }
                            }
                        }
                    }
                    Err(error) => {
                        log::debug!("读取任务栏元素布局失败，保留当前播放器矩形: {error}");
                        stabilizer.retry_after_failure(Instant::now());
                    }
                }
            }

            let has_visible_layout =
                applied_layout.is_some_and(|layout| layout.visible_rect.width() > 0);
            if has_visible_layout
                && (bar_was_suppressed || z_order_refresh_needed || !is_window_visible(bar))
            {
                if show_bar(bar) {
                    bar_was_suppressed = false;
                    z_order_refresh_needed = false;
                } else {
                    retry_needed = true;
                }
            }
        }

        // 首次稳定布局提交前窗口仍是隐藏且非置顶状态，此时不能重置采样状态。
        let topmost_was_applied = applied_layout.is_some();
        if !is_bar_attached_to_taskbar(bar, taskbar)
            || (topmost_was_applied && !is_bar_topmost(bar))
        {
            current_taskbar = HWND::default();
            retry_needed = true;
        }

        match wait_for_relevant_change(
            taskbar,
            retry_needed,
            hook_fallback_needed,
            &stabilizer,
            uia_fallback_needed,
            next_uia_fallback_query,
            &mut fullscreen_monitor,
        ) {
            TaskbarChange::Layout => match active_priority {
                TaskbarOverlapPriority::Bar => immediate_layout_needed = true,
                TaskbarOverlapPriority::TaskbarElements => {
                    stabilizer.invalidate(Instant::now());
                }
            },
            TaskbarChange::Timeout if hook_fallback_needed => match active_priority {
                TaskbarOverlapPriority::Bar => immediate_layout_needed = true,
                TaskbarOverlapPriority::TaskbarElements => stabilizer.invalidate(Instant::now()),
            },
            TaskbarChange::WindowState => z_order_refresh_needed = true,
            TaskbarChange::Timeout => {}
        }
    }
}

/// 等待真实事件或必要兜底到期；全屏查询未变化时继续休眠，不重跑同步流程。
fn wait_for_relevant_change(
    taskbar: HWND,
    retry_needed: bool,
    hook_fallback_needed: bool,
    stabilizer: &LayoutStabilizer,
    uia_fallback_needed: bool,
    next_uia_fallback_query: Instant,
    fullscreen_monitor: &mut FullscreenStateMonitor,
) -> TaskbarChange {
    let recovery_deadline =
        (retry_needed || hook_fallback_needed).then(|| Instant::now() + RECOVERY_RETRY_DELAY);
    loop {
        let now = Instant::now();
        let mut wait_timeout = fullscreen_monitor
            .next_check_at
            .saturating_duration_since(now);
        if let Some(deadline) = recovery_deadline {
            wait_timeout = wait_timeout.min(deadline.saturating_duration_since(now));
        }
        if let Some(deadline) = stabilizer.next_sample_at() {
            wait_timeout = wait_timeout.min(deadline.saturating_duration_since(now));
        }
        if uia_fallback_needed {
            wait_timeout = wait_timeout.min(next_uia_fallback_query.saturating_duration_since(now));
        }

        match wait_for_taskbar_change(wait_timeout) {
            change @ (TaskbarChange::WindowState | TaskbarChange::Layout) => return change,
            TaskbarChange::Timeout => {}
        }

        let now = Instant::now();
        if fullscreen_monitor.refresh_if_due(taskbar, now) {
            return TaskbarChange::WindowState;
        }
        if recovery_deadline.is_some_and(|deadline| now >= deadline)
            || stabilizer.sample_due(now)
            || (uia_fallback_needed && now >= next_uia_fallback_query)
        {
            return TaskbarChange::Timeout;
        }
    }
}

/// 读取一次完整候选布局；UIA 失败会向上传递而不是伪装成空元素列表。
fn measure_layout(
    taskbar: HWND,
    taskbar_rect: ScreenRect,
    priority: TaskbarOverlapPriority,
    taskbar_elements: Option<&TaskbarElements>,
) -> windows::core::Result<BarLayout> {
    let side = resolve_taskbar_side();
    let tray_rect = find_system_tray_rect(taskbar, taskbar_rect);
    let anchor_right = match side {
        TaskbarSide::Left => taskbar_rect.right,
        TaskbarSide::Right => tray_rect.map_or(taskbar_rect.right, |rect| rect.left),
    };
    let dpi = window_dpi(taskbar);
    let ideal_rect = calculate_bar_rect(taskbar_rect, anchor_right, dpi, side, content_width());
    let visible_rect = if priority == TaskbarOverlapPriority::TaskbarElements {
        if let Some(elements) = taskbar_elements {
            hard_clip_bar_rect(
                ideal_rect,
                &elements.button_rects(taskbar_rect, tray_rect)?,
                side,
                dpi,
            )
        } else {
            ideal_rect
        }
    } else {
        ideal_rect
    };

    Ok(BarLayout {
        window_rect: ideal_rect,
        visible_rect,
    })
}

/// 窗口保持理想位置和完整尺寸，仅用 Win32 region 提交稳定后的可见范围。
fn apply_layout(bar: HWND, layout: BarLayout, applied: Option<BarLayout>) -> bool {
    let window_moved = applied.is_none_or(|previous| previous.window_rect != layout.window_rect);
    let old_region_was_clipped = applied
        .is_some_and(|previous| previous.visible_rect.width() < previous.window_rect.width());

    if window_moved {
        if old_region_was_clipped && !clear_bar_clip_before_move(bar) {
            return false;
        }
        if !place_bar(bar, layout.window_rect) {
            return false;
        }
    }

    if layout.visible_rect.width() <= 0 {
        hide_bar(bar);
        true
    } else {
        let clipped = clip_bar(bar, layout.window_rect, layout.visible_rect);
        if clipped && window_moved && old_region_was_clipped {
            redraw_bar(bar);
        }
        clipped
    }
}

/// 清除已经失效的 Explorer 句柄和候选布局，并隐藏播放器。
fn reset_for_missing_taskbar(
    bar: HWND,
    current_taskbar: &mut HWND,
    applied_layout: &mut Option<BarLayout>,
    stabilizer: &mut LayoutStabilizer,
) {
    *current_taskbar = HWND::default();
    *applied_layout = None;
    stabilizer.reset();
    hide_bar(bar);
}
