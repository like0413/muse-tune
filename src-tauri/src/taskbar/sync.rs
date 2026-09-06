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
    TaskbarOverlapPriority,
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
        redraw_bar, show_bar, taskbar_auto_hide_enabled, taskbar_buttons_center_aligned,
        window_dpi,
    },
};

const RECOVERY_RETRY_DELAY: Duration = Duration::from_millis(400);
const WINDOW_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const UIA_FALLBACK_QUERY_INTERVAL: Duration = Duration::from_secs(1);
const MIN_CONTENT_WIDTH_DIP: i32 = 200;
const MAX_CONTENT_WIDTH_DIP: i32 = 360;
static TASKBAR_PLACEMENT: AtomicU8 = AtomicU8::new(TaskbarPlacement::Auto as u8);
static TASKBAR_OVERLAP_PRIORITY: AtomicU8 = AtomicU8::new(TaskbarOverlapPriority::Bar as u8);
static TASKBAR_CONTENT_WIDTH_DIP: AtomicI32 = AtomicI32::new(MAX_CONTENT_WIDTH_DIP);

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
            && is_taskbar_covered_by_fullscreen_window(bar, taskbar, taskbar_rect);
        let taskbar_hidden =
            !is_window_visible(taskbar) || taskbar_rect.width() <= 0 || taskbar_rect.height() <= 2;

        if hidden_for_fullscreen || taskbar_hidden || auto_hide_transitioning {
            hide_bar(bar);
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
            if has_visible_layout && !is_window_visible(bar) {
                show_bar(bar);
            }
        }

        if !is_bar_attached_to_taskbar(bar, taskbar) || !is_bar_topmost(bar) {
            current_taskbar = HWND::default();
            retry_needed = true;
        }

        let mut wait_timeout = if retry_needed || hook_fallback_needed {
            RECOVERY_RETRY_DELAY
        } else {
            WINDOW_HEALTH_CHECK_INTERVAL
        };
        if let Some(deadline) = stabilizer.next_sample_at() {
            wait_timeout = wait_timeout.min(deadline.saturating_duration_since(Instant::now()));
        }
        if uia_fallback_needed {
            wait_timeout =
                wait_timeout.min(next_uia_fallback_query.saturating_duration_since(Instant::now()));
        }

        match wait_for_taskbar_change(wait_timeout) {
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
            TaskbarChange::WindowState | TaskbarChange::Timeout => {}
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
