//! 协调任务栏状态、窗口所有权与播放器布局。

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use windows::Win32::Foundation::HWND;

use super::{
    RECOVERY_RETRY_DELAY, TaskbarOverlapPriority, TaskbarWidthMode,
    elements::TaskbarElements,
    events::{TaskbarChange, WinEventHooks, wait_for_taskbar_change},
    geometry::{
        ScreenRect, TaskbarPlacement, TaskbarSide, auto_content_width, calculate_bar_rect,
        hard_clip_bar_rect, is_rect_within, monitor_rect, window_rect,
    },
    layout::{BarLayout, LayoutStabilizer},
    platform::{
        attach_bar_to_taskbar, clear_bar_clip_before_move, clip_bar, find_system_tray_rect,
        hide_bar, is_bar_attached_to_taskbar, is_bar_topmost,
        is_taskbar_covered_by_fullscreen_window, is_window_alive, is_window_visible, place_bar,
        redraw_bar, shell_reports_fullscreen_activity, show_bar, taskbar_auto_hide_enabled,
        taskbar_buttons_center_aligned, window_dpi,
    },
    settings::{
        content_visible, content_width, horizontal_offset, min_content_width,
        needs_taskbar_elements, overlap_priority, placement, width_mode,
    },
};

/// 全屏状态只有 Shell 查询这一条路径（没有对应的 WinEvent），该间隔就是每块显示器查询它的频率。
/// 调小能让全屏切换后的隐藏与恢复更及时，代价是更高频的跨进程 Shell 查询各自唤醒一整轮同步。
const FULLSCREEN_STATE_CHECK_INTERVAL: Duration = Duration::from_millis(250);
/// 全屏轮询被跳过（系统自动隐藏已开启）时的等待上限；WinEvent 仍可随时唤醒循环。
const IDLE_WAIT_TIMEOUT: Duration = Duration::from_secs(60);
/// UIA 补查频率：事件订阅失败、或 bar 仍处于被裁剪状态时，按该节奏低频重读一次元素矩形，
/// 每次都会重置布局的限频采样。调小能更快跟上元素变化，代价同样是更高频的跨进程 UIA 查询；
/// 订阅可用且布局恢复完整后这条路径自动停止。
const UIA_RECOVERY_QUERY_INTERVAL: Duration = Duration::from_secs(1);

/// 隔离 Shell 未提供事件的全屏状态查询，状态未变化时不唤醒完整同步流程。
struct FullscreenStateMonitor {
    active: bool,
    next_check_at: Instant,
    /// 系统任务栏自动隐藏开启时该状态不会被消费，此时完全跳过 Shell 轮询。
    enabled: bool,
}

impl FullscreenStateMonitor {
    /// 读取初始状态，并安排下一次必要查询。
    fn new(taskbar: HWND, now: Instant) -> Self {
        Self {
            active: shell_reports_fullscreen_activity(taskbar),
            next_check_at: now + FULLSCREEN_STATE_CHECK_INTERVAL,
            enabled: true,
        }
    }

    /// 同步本轮是否真的需要全屏状态；关闭时立即停止查询，重新开启时立刻复查。
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 到期时只读取一次 Shell 状态，并报告状态是否发生变化。
    fn refresh_if_due(&mut self, taskbar: HWND, now: Instant) -> bool {
        if !self.enabled || now < self.next_check_at {
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

/// 将用户偏好和系统任务栏对齐方式解析为实际停靠侧。
fn resolve_taskbar_side() -> TaskbarSide {
    match placement() {
        TaskbarPlacement::Auto if taskbar_buttons_center_aligned() => TaskbarSide::Left,
        TaskbarPlacement::Auto => TaskbarSide::Right,
        TaskbarPlacement::Left => TaskbarSide::Left,
        TaskbarPlacement::Right => TaskbarSide::Right,
    }
}

/// 在窗口存活期间同步任务栏所有权、可见性、位置与硬裁剪方向。
pub(super) fn run(window_handle: isize, taskbar_handle: isize, stop: Arc<AtomicBool>) {
    let taskbar = HWND(taskbar_handle as *mut _);
    // 钩子注册句柄必须活到循环结束，Drop 时才会注销。
    let hooks = WinEventHooks::install(taskbar);
    let mut sync = TaskbarSync::new(window_handle, taskbar, hooks.fallback_needed());

    while !stop.load(Ordering::Acquire) && is_window_alive(sync.bar) {
        if let SyncOutcome::Stop = sync.tick() {
            break;
        }
    }
}

/// 单轮同步的结果；`Stop` 表示任务栏已经消失，循环应当结束。
enum SyncOutcome {
    Continue,
    Stop,
}

/// 任务栏同步循环的独占状态。
///
/// 收敛成结构体后每个阶段只关心自己那几个字段，不必再把同一批可变局部量在函数之间传来传去。
struct TaskbarSync {
    bar: HWND,
    taskbar: HWND,
    hook_fallback_needed: bool,
    taskbar_elements: Option<TaskbarElements>,
    stabilizer: LayoutStabilizer,
    current_taskbar: HWND,
    active_priority: TaskbarOverlapPriority,
    active_width_mode: TaskbarWidthMode,
    applied_layout: Option<BarLayout>,
    needs_elements: bool,
    immediate_layout_needed: bool,
    uia_watch_needed: bool,
    next_uia_recovery_query: Instant,
    bar_was_suppressed: bool,
    z_order_refresh_needed: bool,
    fullscreen_monitor: FullscreenStateMonitor,
}

impl TaskbarSync {
    /// 安装钩子之外的初始状态：元素矩形按需采集，首次布局交给循环去确认。
    fn new(window_handle: isize, taskbar: HWND, hook_fallback_needed: bool) -> Self {
        let now = Instant::now();
        let active_priority = overlap_priority();
        let active_width_mode = width_mode();
        let needs_elements = needs_taskbar_elements(active_priority, active_width_mode);
        let mut sync = Self {
            bar: HWND(window_handle as *mut _),
            taskbar,
            hook_fallback_needed,
            taskbar_elements: None,
            stabilizer: LayoutStabilizer::new(),
            current_taskbar: HWND::default(),
            active_priority,
            active_width_mode,
            applied_layout: None,
            needs_elements,
            immediate_layout_needed: !needs_elements,
            uia_watch_needed: needs_elements,
            next_uia_recovery_query: now + UIA_RECOVERY_QUERY_INTERVAL,
            bar_was_suppressed: true,
            z_order_refresh_needed: true,
            fullscreen_monitor: FullscreenStateMonitor::new(taskbar, now),
        };
        if sync.needs_elements {
            sync.stabilizer.invalidate(now);
        }
        sync
    }

    /// 执行一轮同步：跟随设置变化，校正任务栏所有权，再决定隐藏还是提交布局。
    fn tick(&mut self) -> SyncOutcome {
        let now = Instant::now();
        self.refresh_settings(now);

        if !is_window_alive(self.taskbar) {
            return SyncOutcome::Stop;
        }

        let Some(taskbar_rect) = window_rect(self.taskbar) else {
            self.handle_missing_taskbar();
            let _ = wait_for_taskbar_change(RECOVERY_RETRY_DELAY);
            return SyncOutcome::Continue;
        };

        let attach_retry = self.ensure_attachment();
        let uia_recovery_query_needed = self.refresh_uia_recovery_query(now);
        let auto_hide_enabled = taskbar_auto_hide_enabled();
        let layout_retry = self.sync_visibility_and_layout(taskbar_rect, auto_hide_enabled);
        let detached_retry = self.verify_attachment();
        self.fullscreen_monitor.set_enabled(!auto_hide_enabled);

        match wait_for_relevant_change(
            self.taskbar,
            attach_retry || layout_retry || detached_retry,
            self.hook_fallback_needed,
            &self.stabilizer,
            uia_recovery_query_needed,
            self.next_uia_recovery_query,
            &mut self.fullscreen_monitor,
        ) {
            TaskbarChange::Layout => self.mark_layout_dirty(),
            TaskbarChange::Timeout if self.hook_fallback_needed => self.mark_layout_dirty(),
            TaskbarChange::WindowState => self.z_order_refresh_needed = true,
            TaskbarChange::Timeout => {}
        }
        SyncOutcome::Continue
    }

    /// 跟随避让优先级与宽度模式的变化；依赖元素矩形时立即失效采样，否则立即重算。
    fn refresh_settings(&mut self, now: Instant) {
        let priority = overlap_priority();
        let mode = width_mode();
        if priority == self.active_priority && mode == self.active_width_mode {
            return;
        }
        self.active_priority = priority;
        self.active_width_mode = mode;
        self.needs_elements = needs_taskbar_elements(priority, mode);
        self.stabilizer.reset();
        self.immediate_layout_needed = !self.needs_elements;
        if self.needs_elements {
            self.uia_watch_needed = true;
            self.stabilizer.invalidate(now);
        } else {
            self.taskbar_elements = None;
            self.uia_watch_needed = false;
        }
    }

    /// 清除已经失效的 Explorer 句柄和候选布局，并隐藏播放器。
    fn handle_missing_taskbar(&mut self) {
        self.current_taskbar = HWND::default();
        self.applied_layout = None;
        self.stabilizer.reset();
        hide_bar(self.bar);
        self.bar_was_suppressed = true;
    }

    /// 确保播放器窗口挂在当前任务栏上，并按需开始订阅元素事件。
    ///
    /// 返回本轮是否需要尽快重试。
    fn ensure_attachment(&mut self) -> bool {
        let mut retry_needed = false;
        if self.current_taskbar != self.taskbar
            || !is_bar_attached_to_taskbar(self.bar, self.taskbar)
        {
            attach_bar_to_taskbar(self.bar, self.taskbar);
            if is_bar_attached_to_taskbar(self.bar, self.taskbar) {
                self.current_taskbar = self.taskbar;
                self.uia_watch_needed = self.needs_elements;
            } else {
                retry_needed = true;
            }
            self.applied_layout = None;
            self.stabilizer.reset();
            self.immediate_layout_needed = !self.needs_elements;
            if self.needs_elements {
                self.stabilizer.invalidate(Instant::now());
            }
        }

        if self.uia_watch_needed && self.current_taskbar == self.taskbar {
            if self.taskbar_elements.is_none() {
                self.taskbar_elements = TaskbarElements::new();
            }
            if let Some(elements) = self.taskbar_elements.as_mut() {
                elements.watch_taskbar(self.taskbar);
            }
            self.uia_watch_needed = false;
        }
        retry_needed
    }

    /// 决定本轮是否需要低频校验元素矩形，并推进校验截止时间。
    fn refresh_uia_recovery_query(&mut self, now: Instant) -> bool {
        // 订阅成功只能证明事件处理器已注册；bar 仍被裁剪期间低频校验，恢复后自动停止。
        let applied_layout_is_clipped = self
            .applied_layout
            .is_some_and(|layout| layout.visible_rect.width() < layout.window_rect.width());
        let uia_recovery_query_needed = self.needs_elements
            && (applied_layout_is_clipped
                || self
                    .taskbar_elements
                    .as_ref()
                    .is_some_and(|elements| !elements.is_event_driven()));
        if uia_recovery_query_needed && now >= self.next_uia_recovery_query {
            self.stabilizer.invalidate(now);
            self.next_uia_recovery_query = now + UIA_RECOVERY_QUERY_INTERVAL;
        }
        uia_recovery_query_needed
    }

    /// 按当前可见性决定隐藏还是测量并提交布局；返回本轮是否需要尽快重试。
    fn sync_visibility_and_layout(
        &mut self,
        taskbar_rect: ScreenRect,
        auto_hide_enabled: bool,
    ) -> bool {
        let auto_hide_transitioning = auto_hide_enabled
            && !monitor_rect(self.taskbar)
                .is_some_and(|monitor| is_rect_within(taskbar_rect, monitor));
        let hidden_for_fullscreen = !auto_hide_enabled
            && (is_taskbar_covered_by_fullscreen_window(self.bar, self.taskbar, taskbar_rect)
                || self.fullscreen_monitor.active);
        let taskbar_hidden = !is_window_visible(self.taskbar)
            || taskbar_rect.width() <= 0
            || taskbar_rect.height() <= 2;

        if !content_visible() || hidden_for_fullscreen || taskbar_hidden || auto_hide_transitioning
        {
            hide_bar(self.bar);
            self.bar_was_suppressed = true;
            self.z_order_refresh_needed = true;
            return false;
        }

        let should_measure = if self.needs_elements {
            self.stabilizer.sample_due(Instant::now())
        } else {
            self.immediate_layout_needed || self.applied_layout.is_none()
        };
        let mut retry_needed = false;
        if should_measure {
            match self.measure_layout(taskbar_rect) {
                Ok(candidate) => {
                    let ready = if self.needs_elements {
                        self.stabilizer.observe(candidate, Instant::now())
                    } else {
                        Some(candidate)
                    };
                    if let Some(layout) = ready {
                        if self.applied_layout == Some(layout) {
                            self.immediate_layout_needed = false;
                        } else if self.apply_layout(layout) {
                            self.applied_layout = Some(layout);
                            self.immediate_layout_needed = false;
                        } else {
                            self.applied_layout = None;
                            retry_needed = true;
                            if self.needs_elements {
                                self.stabilizer.invalidate(Instant::now());
                            }
                        }
                    }
                }
                Err(error) => {
                    log::debug!("读取任务栏元素布局失败，保留当前播放器矩形: {error}");
                    self.stabilizer.retry_after_failure(Instant::now());
                }
            }
        }

        let has_visible_layout = self
            .applied_layout
            .is_some_and(|layout| layout.visible_rect.width() > 0);
        if has_visible_layout
            && (self.bar_was_suppressed
                || self.z_order_refresh_needed
                || !is_window_visible(self.bar))
        {
            if show_bar(self.bar) {
                self.bar_was_suppressed = false;
                self.z_order_refresh_needed = false;
            } else {
                retry_needed = true;
            }
        }
        retry_needed
    }

    /// 校验窗口仍挂在任务栏上且保持置顶；失效时下一轮重新挂载。
    fn verify_attachment(&mut self) -> bool {
        // 首次稳定布局提交前窗口仍是隐藏且非置顶状态，此时不能重置采样状态。
        let topmost_was_applied = self.applied_layout.is_some();
        if !is_bar_attached_to_taskbar(self.bar, self.taskbar)
            || (topmost_was_applied && !is_bar_topmost(self.bar))
        {
            self.current_taskbar = HWND::default();
            return true;
        }
        false
    }

    /// 标记布局需要重算：依赖任务栏元素矩形时走限频采样，否则立即应用。
    fn mark_layout_dirty(&mut self) {
        if self.needs_elements {
            self.stabilizer.invalidate(Instant::now());
        } else {
            self.immediate_layout_needed = true;
        }
    }

    /// 读取一次完整候选布局；UIA 失败会向上传递而不是伪装成空元素列表。
    fn measure_layout(&self, taskbar_rect: ScreenRect) -> windows::core::Result<BarLayout> {
        let side = resolve_taskbar_side();
        let tray_rect = find_system_tray_rect(self.taskbar, taskbar_rect);
        let anchor_right = match side {
            TaskbarSide::Left => taskbar_rect.right,
            TaskbarSide::Right => tray_rect.map_or(taskbar_rect.right, |rect| rect.left),
        };
        let dpi = window_dpi(self.taskbar);
        // 避让任务栏元素和自适应宽度都需要元素矩形，因此只读取一次。
        let button_rects = self
            .taskbar_elements
            .as_ref()
            .map(|elements| elements.button_rects(taskbar_rect, tray_rect));
        let content_width = match self.active_width_mode {
            TaskbarWidthMode::Fixed => content_width(),
            TaskbarWidthMode::Auto => auto_width_from_elements(
                button_rects.as_ref(),
                taskbar_rect,
                anchor_right,
                side,
                dpi,
            ),
        };
        let ideal_rect = calculate_bar_rect(
            taskbar_rect,
            anchor_right,
            dpi,
            side,
            content_width,
            horizontal_offset(),
        );
        let visible_rect = if self.active_priority == TaskbarOverlapPriority::TaskbarElements {
            match button_rects {
                Some(rects) => hard_clip_bar_rect(ideal_rect, &rects?, side, dpi),
                None => ideal_rect,
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
    fn apply_layout(&self, layout: BarLayout) -> bool {
        let window_moved = self
            .applied_layout
            .is_none_or(|previous| previous.window_rect != layout.window_rect);
        let old_region_was_clipped = self
            .applied_layout
            .is_some_and(|previous| previous.visible_rect.width() < previous.window_rect.width());

        if window_moved {
            if old_region_was_clipped && !clear_bar_clip_before_move(self.bar) {
                return false;
            }
            if !place_bar(self.bar, layout.window_rect) {
                return false;
            }
        }

        if layout.visible_rect.width() <= 0 {
            hide_bar(self.bar);
            true
        } else {
            let clipped = clip_bar(self.bar, layout.window_rect, layout.visible_rect);
            if clipped && window_moved && old_region_was_clipped {
                redraw_bar(self.bar);
            }
            clipped
        }
    }
}

/// 等待真实事件或必要兜底到期；全屏查询未变化时继续休眠，不重跑同步流程。
fn wait_for_relevant_change(
    taskbar: HWND,
    retry_needed: bool,
    hook_fallback_needed: bool,
    stabilizer: &LayoutStabilizer,
    uia_recovery_query_needed: bool,
    next_uia_recovery_query: Instant,
    fullscreen_monitor: &mut FullscreenStateMonitor,
) -> TaskbarChange {
    let recovery_deadline =
        (retry_needed || hook_fallback_needed).then(|| Instant::now() + RECOVERY_RETRY_DELAY);
    loop {
        let now = Instant::now();
        // 系统任务栏自动隐藏开启时全屏状态结果不会被消费，因此不把它的截止时间纳入等待，
        // 从而消除每显示器 4Hz 的 Shell 状态查询；WinEvent 仍能立即唤醒本循环。
        let mut wait_timeout = if fullscreen_monitor.enabled {
            fullscreen_monitor
                .next_check_at
                .saturating_duration_since(now)
        } else {
            IDLE_WAIT_TIMEOUT
        };
        if let Some(deadline) = recovery_deadline {
            wait_timeout = wait_timeout.min(deadline.saturating_duration_since(now));
        }
        if let Some(deadline) = stabilizer.next_sample_at() {
            wait_timeout = wait_timeout.min(deadline.saturating_duration_since(now));
        }
        if uia_recovery_query_needed {
            wait_timeout = wait_timeout.min(next_uia_recovery_query.saturating_duration_since(now));
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
            || (uia_recovery_query_needed && now >= next_uia_recovery_query)
        {
            return TaskbarChange::Timeout;
        }
    }
}

/// 自适应宽度：元素矩形可用时取停靠侧空白；不可用时退回用户设定的固定宽度，
/// 避免 UIA 暂时不可用导致自适应模式下 bar 直接消失。空白不足时按最小宽度兜底。
fn auto_width_from_elements(
    button_rects: Option<&windows::core::Result<Vec<ScreenRect>>>,
    taskbar_rect: ScreenRect,
    anchor_right: i32,
    side: TaskbarSide,
    dpi: u32,
) -> i32 {
    let Some(Ok(elements)) = button_rects else {
        return content_width();
    };

    auto_content_width(taskbar_rect, anchor_right, side, dpi, elements).max(min_content_width())
}
