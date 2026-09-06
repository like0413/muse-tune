//! 通过 WinEvent 与线程消息循环感知任务栏及前台窗口变化。

use std::{
    cell::Cell,
    collections::HashMap,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
        WindowsAndMessaging::{
            DispatchMessageW, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
            EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_REORDER, EVENT_OBJECT_SHOW,
            EVENT_SYSTEM_FOREGROUND, GA_ROOT, GetAncestor, GetForegroundWindow, MSG,
            MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, OBJID_WINDOW, PM_NOREMOVE, PM_REMOVE,
            PeekMessageW, PostThreadMessageW, QS_ALLINPUT, TranslateMessage, WINEVENT_OUTOFCONTEXT,
            WINEVENT_SKIPOWNPROCESS, WM_APP,
        },
    },
};

thread_local! {
    static WINDOW_STATE_CHANGED: Cell<bool> = const { Cell::new(false) };
    static TASKBAR_LAYOUT_CHANGED: Cell<bool> = const { Cell::new(false) };
    static WATCHED_TASKBAR: Cell<HWND> = const { Cell::new(HWND(std::ptr::null_mut())) };
}
static MONITOR_THREADS: LazyLock<Mutex<HashMap<u32, Arc<AtomicBool>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 在当前同步线程上合并布局失效状态。
fn mark_layout_update() {
    let thread_id = unsafe { GetCurrentThreadId() };
    if let Ok(threads) = MONITOR_THREADS.lock()
        && let Some(pending) = threads.get(&thread_id)
    {
        pending.store(false, Ordering::Release);
    }
    TASKBAR_LAYOUT_CHANGED.set(true);
    WINDOW_STATE_CHANGED.set(true);
}

/// 将 UIA 布局变化定向投递给所属显示器的同步线程。
pub(super) fn request_layout_update(thread_id: u32) {
    let pending = MONITOR_THREADS
        .lock()
        .ok()
        .and_then(|threads| threads.get(&thread_id).cloned());
    let Some(pending) = pending else {
        return;
    };
    if pending.swap(true, Ordering::AcqRel) {
        return;
    }

    // SAFETY: 消息不携带指针或资源所有权，线程退出后投递失败也无副作用。
    if unsafe { PostThreadMessageW(thread_id, WM_APP, WPARAM(0), LPARAM(0)) }.is_err() {
        pending.store(false, Ordering::Release);
    }
}

/// 唤醒所有显示器对应的同步线程，使全局设置立即生效。
pub(super) fn request_all_layout_updates() {
    let Ok(thread_ids) = MONITOR_THREADS
        .lock()
        .map(|threads| threads.keys().copied().collect::<Vec<_>>())
    else {
        return;
    };
    for thread_id in thread_ids {
        request_layout_update(thread_id);
    }
}

pub(super) enum TaskbarChange {
    WindowState,
    Layout,
    Timeout,
}

/// 持有当前线程安装的 WinEvent 钩子，并在离开同步循环时自动释放。
pub(super) struct WinEventHooks {
    foreground: Option<HWINEVENTHOOK>,
    structure: Option<HWINEVENTHOOK>,
    location: Option<HWINEVENTHOOK>,
}

impl WinEventHooks {
    /// 安装前台、窗口结构和位置变化钩子。
    pub(super) fn install(taskbar: HWND) -> Self {
        let mut message = MSG::default();
        // SAFETY: 空范围 PeekMessage 仅确保当前线程拥有消息队列，不移除或分发消息。
        let _ = unsafe { PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE) };
        // SAFETY: 当前函数运行在唯一的任务栏监控线程上。
        let thread_id = unsafe { GetCurrentThreadId() };
        WATCHED_TASKBAR.set(taskbar);
        if let Ok(mut threads) = MONITOR_THREADS.lock() {
            threads.insert(thread_id, Arc::new(AtomicBool::new(false)));
        }

        Self {
            foreground: install_win_event_hook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
            structure: install_win_event_hook(EVENT_OBJECT_CREATE, EVENT_OBJECT_REORDER),
            location: install_win_event_hook(
                EVENT_OBJECT_LOCATIONCHANGE,
                EVENT_OBJECT_LOCATIONCHANGE,
            ),
        }
    }

    /// 任一钩子失败时启用更短的超时轮询作为恢复兜底。
    pub(super) fn fallback_needed(&self) -> bool {
        self.foreground.is_none() || self.structure.is_none() || self.location.is_none()
    }
}

impl Drop for WinEventHooks {
    fn drop(&mut self) {
        let thread_id = unsafe { GetCurrentThreadId() };
        if let Ok(mut threads) = MONITOR_THREADS.lock() {
            threads.remove(&thread_id);
        }
        WATCHED_TASKBAR.set(HWND::default());
        uninstall_win_event_hook(self.foreground.take());
        uninstall_win_event_hook(self.structure.take());
        uninstall_win_event_hook(self.location.take());
    }
}

/// 等待窗口状态、任务栏布局变化或健康检查超时。
pub(super) fn wait_for_taskbar_change(timeout: Duration) -> TaskbarChange {
    // 进程外 WinEvent 会投递到安装钩子的线程。消息感知等待可立即响应事件；超时仅用于
    // 检查窗口是否存活，以及在 API 或钩子失效后加快恢复，不参与 bar 动画。
    let deadline = Instant::now() + timeout;
    loop {
        if WINDOW_STATE_CHANGED.replace(false) {
            return take_pending_taskbar_change();
        }

        let wait_millis = deadline
            .saturating_duration_since(Instant::now())
            .as_millis() as u32;
        let _ = unsafe {
            MsgWaitForMultipleObjectsEx(None, wait_millis, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
        };

        let mut message = MSG::default();
        // SAFETY: `message` 在每次调用期间均为有效存储，消息也在所属线程分发。
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() } {
            if message.message == WM_APP {
                mark_layout_update();
                continue;
            }
            let _ = unsafe { TranslateMessage(&message) };
            unsafe { DispatchMessageW(&message) };
        }

        if WINDOW_STATE_CHANGED.replace(false) {
            return take_pending_taskbar_change();
        }
        if Instant::now() >= deadline {
            return TaskbarChange::Timeout;
        }
    }
}

/// 安装指定范围的进程外 WinEvent 钩子。
fn install_win_event_hook(event_min: u32, event_max: u32) -> Option<HWINEVENTHOOK> {
    // SAFETY: 回调是静态函数，进程外投递模式仍在本进程执行；当前监控线程在钩子存续期间持有 Win32 消息循环。
    let hook = unsafe {
        SetWinEventHook(
            event_min,
            event_max,
            None,
            Some(handle_win_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    (!hook.is_invalid()).then_some(hook)
}

/// 释放由当前线程安装的 WinEvent 钩子。
fn uninstall_win_event_hook(hook: Option<HWINEVENTHOOK>) {
    if let Some(hook) = hook {
        // SAFETY: 钩子由当前监控线程创建，并且只在这里释放一次。
        let _ = unsafe { UnhookWinEvent(hook) };
    }
}

/// 将 WinEvent 回调归并为同步循环需要的两类状态信号。
unsafe extern "system" fn handle_win_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    object_id: i32,
    _child_id: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND {
        WINDOW_STATE_CHANGED.set(true);
        return;
    }
    if window.0.is_null() || (event == EVENT_OBJECT_LOCATIONCHANGE && object_id != OBJID_WINDOW.0) {
        return;
    }

    // SAFETY: 此调用只读取 WinEvent 回调提供的借用句柄。
    let root = unsafe { GetAncestor(window, GA_ROOT) };
    let is_taskbar_event = WATCHED_TASKBAR.get() == root;
    let taskbar_location_changed =
        is_taskbar_event && event == EVENT_OBJECT_LOCATIONCHANGE && object_id == OBJID_WINDOW.0;
    let taskbar_structure_changed = is_taskbar_event
        && matches!(
            event,
            EVENT_OBJECT_CREATE
                | EVENT_OBJECT_DESTROY
                | EVENT_OBJECT_SHOW
                | EVENT_OBJECT_HIDE
                | EVENT_OBJECT_REORDER
        );
    // SAFETY: 返回的前台窗口句柄仅按值比较，不接管其所有权。
    let foreground_geometry_changed = event == EVENT_OBJECT_LOCATIONCHANGE
        && window == root
        && root == unsafe { GetForegroundWindow() };

    if foreground_geometry_changed || taskbar_location_changed || taskbar_structure_changed {
        WINDOW_STATE_CHANGED.set(true);
    }
    if taskbar_location_changed || taskbar_structure_changed {
        TASKBAR_LAYOUT_CHANGED.set(true);
    }
}

/// 优先返回更具体的任务栏布局变化信号。
fn take_pending_taskbar_change() -> TaskbarChange {
    if TASKBAR_LAYOUT_CHANGED.replace(false) {
        TaskbarChange::Layout
    } else {
        TaskbarChange::WindowState
    }
}
