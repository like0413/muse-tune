//! 协调任务栏状态、窗口所有权与播放器布局。

use std::time::Duration;

use windows::Win32::Foundation::HWND;

use super::{
    events::{TaskbarChange, WinEventHooks, wait_for_taskbar_change},
    geometry::{TaskbarSide, calculate_bar_rect, is_rect_within, monitor_rect, window_rect},
    platform::{
        attach_bar_to_taskbar, find_primary_taskbar, find_system_tray_left_edge, hide_bar,
        is_bar_attached_to_taskbar, is_bar_topmost, is_taskbar_covered_by_fullscreen_window,
        is_window_alive, is_window_visible, place_bar, show_bar, taskbar_auto_hide_enabled,
        window_dpi,
    },
};

const RECOVERY_RETRY_DELAY: Duration = Duration::from_millis(400);
const WINDOW_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_TASKBAR_SIDE: TaskbarSide = TaskbarSide::from_right_aligned(false);

/// 在窗口存活期间同步任务栏所有权、可见性和位置。
pub(super) fn run(window_handle: isize) {
    let hooks = WinEventHooks::install();
    let hook_fallback_needed = hooks.fallback_needed();

    let bar = HWND(window_handle as *mut _);
    let mut current_owner = HWND::default();
    let mut last_bar_rect = None;
    let mut placement_needs_update = true;

    while is_window_alive(bar) {
        let mut retry_needed = false;
        let Some(taskbar) = find_primary_taskbar() else {
            current_owner = HWND::default();
            last_bar_rect = None;
            hide_bar(bar);
            let _ = wait_for_taskbar_change(RECOVERY_RETRY_DELAY);
            continue;
        };

        let Some(taskbar_rect) = window_rect(taskbar) else {
            current_owner = HWND::default();
            last_bar_rect = None;
            hide_bar(bar);
            let _ = wait_for_taskbar_change(RECOVERY_RETRY_DELAY);
            continue;
        };

        if current_owner != taskbar {
            attach_bar_to_taskbar(bar, taskbar);
            if is_bar_attached_to_taskbar(bar, taskbar) {
                current_owner = taskbar;
            } else {
                retry_needed = true;
            }
            last_bar_rect = None;
            placement_needs_update = true;
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
            let mut placement_changed = false;
            if placement_needs_update || last_bar_rect.is_none() {
                let anchor_right = match DEFAULT_TASKBAR_SIDE {
                    TaskbarSide::Left => taskbar_rect.right,
                    TaskbarSide::Right => find_system_tray_left_edge(taskbar, taskbar_rect)
                        .unwrap_or(taskbar_rect.right),
                };
                let bar_rect = calculate_bar_rect(
                    taskbar_rect,
                    anchor_right,
                    window_dpi(taskbar),
                    DEFAULT_TASKBAR_SIDE,
                );
                placement_changed = last_bar_rect != Some(bar_rect);
                if !placement_changed || place_bar(bar, bar_rect) {
                    last_bar_rect = Some(bar_rect);
                    placement_needs_update = false;
                } else {
                    last_bar_rect = None;
                    retry_needed = true;
                }
            }

            if last_bar_rect.is_some() && !placement_changed && !is_window_visible(bar) {
                show_bar(bar);
            }
        }

        if !is_bar_attached_to_taskbar(bar, taskbar) || !is_bar_topmost(bar) {
            current_owner = HWND::default();
            retry_needed = true;
        }

        let wait_timeout = if retry_needed || hook_fallback_needed {
            RECOVERY_RETRY_DELAY
        } else {
            WINDOW_HEALTH_CHECK_INTERVAL
        };
        match wait_for_taskbar_change(wait_timeout) {
            TaskbarChange::Layout => placement_needs_update = true,
            TaskbarChange::Timeout if hook_fallback_needed => placement_needs_update = true,
            TaskbarChange::WindowState | TaskbarChange::Timeout => {}
        }
    }
}
