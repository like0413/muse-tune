//! 使用 Win32 所有者窗口关系，将 Tauri bar 窗口集成到 Windows 任务栏。

mod displays;
mod elements;
mod events;
mod geometry;
mod layout;
mod platform;
mod sync;
mod volume_popup;

use std::{
    collections::HashMap,
    sync::{
        Arc, Condvar, LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use tauri::{AppHandle, Manager, Runtime, WebviewWindow};
use tauri_plugin_store::StoreExt;

use crate::native_defaults;
use crate::settings_store::PATH as SETTINGS_STORE_PATH;

pub use displays::TaskbarDisplay;
pub use geometry::TaskbarPlacement;
const TASKBAR_WINDOW_LABEL: &str = "taskbar";
const RECOVERY_RETRY_DELAY: Duration = Duration::from_millis(400);
const DISPLAY_TOPOLOGY_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const DISPLAY_TARGET_KEY: &str = "taskbar.displayTarget";
const WIDTH_KEY: &str = "taskbar.width";
const WIDTH_MODE_KEY: &str = "taskbar.widthMode";
const PLACEMENT_KEY: &str = "taskbar.placement";
const OVERLAP_PRIORITY_KEY: &str = "taskbar.overlapPriority";
static TASKBAR_CONTENT_VISIBLE: AtomicBool = AtomicBool::new(true);

struct DisplayTargetState {
    value: String,
    revision: u64,
}

static DISPLAY_TARGET: LazyLock<(Mutex<DisplayTargetState>, Condvar)> = LazyLock::new(|| {
    (
        Mutex::new(DisplayTargetState {
            value: native_defaults::display_target().to_owned(),
            revision: 0,
        }),
        Condvar::new(),
    )
});

struct ManagedBar<R: Runtime> {
    window: WebviewWindow<R>,
    taskbar: isize,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl<R: Runtime> Drop for ManagedBar<R> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        events::request_all_layout_updates();
        if let Some(worker) = self.worker.take() {
            if worker.thread().id() == thread::current().id() {
                log::error!("任务栏同步 worker 尝试等待自身，已跳过 join");
            } else if worker.join().is_err() {
                log::warn!("任务栏同步线程异常退出");
            }
        }
        // 保持窗口存活直至 worker 释放 UIA、WinEvent 和借用 HWND。
        let _ = self.window.close();
    }
}

/// 拥有任务栏窗口管理线程，并协调应用退出时的停止与回收。
pub(crate) struct TaskbarService {
    stop: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl TaskbarService {
    /// 非阻塞通知 monitor 和所有 sync worker 结束等待。
    pub(crate) fn request_shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        DISPLAY_TARGET.1.notify_all();
        events::request_all_layout_updates();
    }

    /// 等待 monitor 回收所有 bar worker 及其原生资源。
    pub(crate) fn shutdown(&self) {
        self.request_shutdown();
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(worker) = worker {
            if worker.thread().id() == thread::current().id() {
                log::error!("任务栏 monitor 尝试等待自身，已跳过 join");
                return;
            }
            if worker.join().is_err() {
                log::warn!("任务栏窗口管理线程异常退出");
            }
        }
    }
}

impl Drop for TaskbarService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize)]
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize)]
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

/// 更新播放器定位偏好，并通知监控线程立即重新计算位置。
pub fn set_placement(placement: TaskbarPlacement) {
    sync::set_placement(placement);
}

/// 更新任务栏元素与播放器的遮挡优先级，并通知监控线程重新计算可见区域。
pub fn set_overlap_priority(priority: TaskbarOverlapPriority) {
    sync::set_overlap_priority(priority);
}

/// 更新 bar 基准宽度，并通知监控线程立即重新计算位置与裁剪区域。
pub fn set_content_width(width: i32) {
    sync::set_content_width(width);
}

/// 更新 bar 宽度模式（固定宽度或自适应），并通知监控线程重新计算布局。
pub fn set_width_mode(mode: TaskbarWidthMode) {
    sync::set_width_mode(mode);
}

/// 更新 bar 内容可见性；值变化时立即唤醒全部同步线程。
pub fn set_content_visibility(visible: bool) {
    if TASKBAR_CONTENT_VISIBLE.swap(visible, Ordering::AcqRel) != visible {
        events::request_all_layout_updates();
    }
}

/// 读取媒体状态计算出的 bar 内容可见性。
pub(crate) fn content_visible() -> bool {
    TASKBAR_CONTENT_VISIBLE.load(Ordering::Acquire)
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
    let target = display_target_snapshot().0;
    let (placement, overlap_priority, width_mode, width) = sync::diagnostic_settings();
    (target, placement, overlap_priority, width_mode, width)
}

/// 显示并定位独立音量悬浮窗。
pub fn show_volume_popup<R: Runtime>(
    source: &WebviewWindow<R>,
    anchor_center_x: f64,
    theme_color: String,
) -> Result<(), String> {
    volume_popup::show(source, anchor_center_x, theme_color)
}

/// 仅允许音量悬浮窗隐藏自身。
pub fn hide_volume_popup<R: Runtime>(
    source: &WebviewWindow<R>,
    generation: u64,
) -> Result<(), String> {
    volume_popup::hide(source, generation)
}

/// 更新目标显示器，并立即唤醒窗口管理线程。
pub fn set_display_target(target: String) -> Result<(), String> {
    if target != native_defaults::display_target() && (target.is_empty() || target.trim() != target)
    {
        return Err("目标显示器标识无效".to_owned());
    }

    let (state, changed) = &*DISPLAY_TARGET;
    let mut state = state
        .lock()
        .map_err(|_| "目标显示器状态不可用".to_owned())?;
    if state.value != target {
        state.value = target;
        mark_display_state_changed(&mut state, changed);
    }
    Ok(())
}

/// 接收任务栏 WinEvent，立即要求窗口管理线程重新枚举显示器。
pub(super) fn request_display_refresh() {
    let (state, changed) = &*DISPLAY_TARGET;
    if let Ok(mut state) = state.lock() {
        mark_display_state_changed(&mut state, changed);
    }
}

/// 增加显示器状态版本并唤醒唯一的窗口管理线程。
fn mark_display_state_changed(state: &mut DisplayTargetState, changed: &Condvar) {
    state.revision = state.revision.wrapping_add(1);
    changed.notify_one();
}

/// 启动独立监控线程，持续维护任务栏与播放器窗口的所有者关系。
pub fn initialize<R: Runtime>(
    app: &mut tauri::App<R>,
) -> Result<TaskbarService, Box<dyn std::error::Error>> {
    restore_native_settings(app);
    let app_handle = app.handle().clone();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);

    let worker = thread::Builder::new()
        .name("taskbar-monitor".to_owned())
        .spawn(move || maintain_bar_windows(app_handle, worker_stop))?;

    Ok(TaskbarService {
        stop,
        worker: Mutex::new(Some(worker)),
    })
}

/// 在创建动态 bar 前直接从官方 Store 恢复原生窗口所需的全局设置。
fn restore_native_settings<R: Runtime>(app: &tauri::App<R>) {
    let store = match app.store(SETTINGS_STORE_PATH) {
        Ok(store) => store,
        Err(error) => {
            log::warn!("读取持久化设置失败，本次启动使用默认任务栏设置: {error}");
            return;
        }
    };

    if let Some(placement) = store
        .get(PLACEMENT_KEY)
        .and_then(|value| serde_json::from_value::<TaskbarPlacement>(value).ok())
    {
        set_placement(placement);
    }
    if let Some(width) = store
        .get(WIDTH_KEY)
        .and_then(|value| value.as_i64())
        .and_then(|value| i32::try_from(value).ok())
    {
        set_content_width(width);
    }
    if let Some(mode) = store
        .get(WIDTH_MODE_KEY)
        .and_then(|value| serde_json::from_value::<TaskbarWidthMode>(value).ok())
    {
        set_width_mode(mode);
    }
    if let Some(priority) = store
        .get(OVERLAP_PRIORITY_KEY)
        .and_then(|value| serde_json::from_value::<TaskbarOverlapPriority>(value).ok())
    {
        set_overlap_priority(priority);
    }
    if let Some(target) = store
        .get(DISPLAY_TARGET_KEY)
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|target| {
            target == native_defaults::display_target()
                || (!target.is_empty() && target.trim() == target)
        })
        && let Err(error) = set_display_target(target)
    {
        log::warn!("恢复目标显示器失败，本次启动使用全部显示器: {error}");
    }
}

/// 获取或重建播放器窗口，并在 Explorer 生命周期内持续恢复同步。
fn maintain_bar_windows<R: Runtime>(app: AppHandle<R>, stop: Arc<AtomicBool>) {
    let Some(base_config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == TASKBAR_WINDOW_LABEL)
        .cloned()
    else {
        return;
    };
    let mut bars: HashMap<String, ManagedBar<R>> = HashMap::new();

    while !stop.load(Ordering::Acquire) {
        let (target, observed_revision) = display_target_snapshot();
        let displays = displays::taskbar_displays();
        let primary_was_managed = bars
            .values()
            .any(|bar| bar.window.label() == TASKBAR_WINDOW_LABEL);
        let mut stopped_bar = false;

        bars.retain(|id, bar| {
            let bar_is_alive = bar.window.hwnd().is_ok_and(platform::is_window_alive);
            let remains_selected = displays.iter().any(|display| {
                bar_is_alive
                    && display.display.id == *id
                    && display.taskbar == bar.taskbar
                    && (target == native_defaults::display_target() || target == *id)
            });
            if !remains_selected {
                stopped_bar = true;
            }
            remains_selected
        });
        if stopped_bar {
            events::request_all_layout_updates();
        }

        let primary_is_selected = displays.iter().any(|display| {
            display.display.is_primary
                && (target == native_defaults::display_target() || target == display.display.id)
        });
        if !primary_is_selected
            && !primary_was_managed
            && let Some(window) = app.get_webview_window(TASKBAR_WINDOW_LABEL)
        {
            // 启动配置会预建主 bar；单独选择副屏时不应保留一个隐藏 WebView。
            let _ = window.close();
        }

        for display in displays.into_iter().filter(|display| {
            target == native_defaults::display_target() || target == display.display.id
        }) {
            if bars.contains_key(&display.display.id) {
                continue;
            }

            let label = window_label(&display.display.id, display.display.is_primary);
            let mut config = base_config.clone();
            config.label.clone_from(&label);
            let window = if let Some(window) = app.get_webview_window(&label) {
                window
            } else {
                let Ok(builder) = tauri::WebviewWindowBuilder::from_config(&app, &config) else {
                    continue;
                };
                let Ok(window) = builder.build() else {
                    continue;
                };
                window
            };
            let Ok(window_handle) = window.hwnd() else {
                let _ = window.close();
                continue;
            };

            let taskbar = display.taskbar;
            let bar = window_handle.0 as isize;
            let stop = Arc::new(AtomicBool::new(false));
            let worker_stop = Arc::clone(&stop);
            let worker = thread::Builder::new()
                .name(format!("taskbar-sync-{label}"))
                .spawn(move || sync::run(bar, taskbar, worker_stop));
            let Ok(worker) = worker else {
                let _ = window.close();
                continue;
            };
            bars.insert(
                display.display.id,
                ManagedBar {
                    window,
                    taskbar,
                    stop,
                    worker: Some(worker),
                },
            );
        }

        wait_for_display_change(observed_revision, !bars.is_empty(), &stop);
    }
}

/// 原子读取目标与修订号，避免设置变化发生在两次独立读取之间。
fn display_target_snapshot() -> (String, u64) {
    DISPLAY_TARGET.0.lock().map_or_else(
        |_| (native_defaults::display_target().to_owned(), 0),
        |state| (state.value.clone(), state.revision),
    )
}

/// 优先等待设置或 WinEvent；没有可用事件源时才低频轮询恢复任务栏。
fn wait_for_display_change(observed_revision: u64, has_managed_bar: bool, stop: &AtomicBool) {
    let Ok(state) = DISPLAY_TARGET.0.lock() else {
        thread::sleep(RECOVERY_RETRY_DELAY);
        return;
    };
    let event_driven = has_managed_bar && events::topology_events_available();
    let wait_failed = if event_driven {
        DISPLAY_TARGET
            .1
            .wait_while(state, |state| {
                state.revision == observed_revision && !stop.load(Ordering::Acquire)
            })
            .is_err()
    } else {
        DISPLAY_TARGET
            .1
            .wait_timeout_while(state, DISPLAY_TOPOLOGY_CHECK_INTERVAL, |state| {
                state.revision == observed_revision && !stop.load(Ordering::Acquire)
            })
            .is_err()
    };
    if wait_failed {
        thread::sleep(RECOVERY_RETRY_DELAY);
    }
}

/// 为动态 bar 生成受 capability 通配规则约束的窗口标签。
fn window_label(display_id: &str, is_primary: bool) -> String {
    if is_primary {
        return TASKBAR_WINDOW_LABEL.to_owned();
    }

    let suffix: String = display_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect();
    format!("taskbar-{suffix}")
}
