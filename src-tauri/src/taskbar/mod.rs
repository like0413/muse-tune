//! 使用 Win32 所有者窗口关系，将 Tauri bar 窗口集成到 Windows 任务栏。

mod displays;
mod elements;
mod events;
mod geometry;
mod layout;
mod platform;
mod sync;

use std::{
    collections::HashMap,
    sync::{
        Arc, Condvar, LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use tauri::{AppHandle, Manager, Runtime, WebviewWindow};
use tauri_plugin_store::StoreExt;

pub use displays::TaskbarDisplay;
pub use geometry::TaskbarPlacement;
const TASKBAR_WINDOW_LABEL: &str = "taskbar";
const RECOVERY_RETRY_DELAY: Duration = Duration::from_millis(400);
const DISPLAY_TOPOLOGY_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const ALL_DISPLAYS: &str = "all";
const SETTINGS_STORE_PATH: &str = "settings.json";
const DISPLAY_TARGET_KEY: &str = "taskbar.displayTarget";
const WIDTH_KEY: &str = "taskbar.width";
const PLACEMENT_KEY: &str = "taskbar.placement";
const OVERLAP_PRIORITY_KEY: &str = "taskbar.overlapPriority";

struct DisplayTargetState {
    value: String,
    revision: u64,
}

static DISPLAY_TARGET: LazyLock<(Mutex<DisplayTargetState>, Condvar)> = LazyLock::new(|| {
    (
        Mutex::new(DisplayTargetState {
            value: ALL_DISPLAYS.to_owned(),
            revision: 0,
        }),
        Condvar::new(),
    )
});

struct ManagedBar<R: Runtime> {
    window: WebviewWindow<R>,
    taskbar: isize,
    stop: Arc<AtomicBool>,
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

/// 返回当前拥有 Windows 任务栏的显示器。
pub fn available_displays() -> Vec<TaskbarDisplay> {
    displays::available_taskbar_displays()
}

/// 更新目标显示器，并立即唤醒窗口管理线程。
pub fn set_display_target(target: String) -> Result<(), String> {
    if target != ALL_DISPLAYS && (target.is_empty() || target.trim() != target) {
        return Err("目标显示器标识无效".to_owned());
    }

    let (state, changed) = &*DISPLAY_TARGET;
    let mut state = state
        .lock()
        .map_err(|_| "目标显示器状态不可用".to_owned())?;
    if state.value != target {
        state.value = target;
        state.revision = state.revision.wrapping_add(1);
        changed.notify_one();
    }
    Ok(())
}

/// 启动独立监控线程，持续维护任务栏与播放器窗口的所有者关系。
pub fn initialize<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    restore_native_settings(app);
    let app_handle = app.handle().clone();

    thread::Builder::new()
        .name("taskbar-monitor".to_owned())
        .spawn(move || maintain_bar_windows(app_handle))?;

    Ok(())
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
    if let Some(priority) = store
        .get(OVERLAP_PRIORITY_KEY)
        .and_then(|value| serde_json::from_value::<TaskbarOverlapPriority>(value).ok())
    {
        set_overlap_priority(priority);
    }
    if let Some(target) = store
        .get(DISPLAY_TARGET_KEY)
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|target| target == ALL_DISPLAYS || (!target.is_empty() && target.trim() == target))
        && let Err(error) = set_display_target(target)
    {
        log::warn!("恢复目标显示器失败，本次启动使用全部显示器: {error}");
    }
}

/// 获取或重建播放器窗口，并在 Explorer 生命周期内持续恢复同步。
fn maintain_bar_windows<R: Runtime>(app: AppHandle<R>) {
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

    loop {
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
                    && (target == ALL_DISPLAYS || target == *id)
            });
            if !remains_selected {
                bar.stop.store(true, Ordering::Release);
                let _ = bar.window.close();
                stopped_bar = true;
            }
            remains_selected
        });
        if stopped_bar {
            events::request_all_layout_updates();
        }

        let primary_is_selected = displays.iter().any(|display| {
            display.display.is_primary && (target == ALL_DISPLAYS || target == display.display.id)
        });
        if !primary_is_selected
            && !primary_was_managed
            && let Some(window) = app.get_webview_window(TASKBAR_WINDOW_LABEL)
        {
            // 启动配置会预建主 bar；单独选择副屏时不应保留一个隐藏 WebView。
            let _ = window.close();
        }

        for display in displays
            .into_iter()
            .filter(|display| target == ALL_DISPLAYS || target == display.display.id)
        {
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
            if worker.is_err() {
                let _ = window.close();
                continue;
            }
            bars.insert(
                display.display.id,
                ManagedBar {
                    window,
                    taskbar,
                    stop,
                },
            );
        }

        wait_for_display_change(observed_revision);
    }
}

/// 原子读取目标与修订号，避免设置变化发生在两次独立读取之间。
fn display_target_snapshot() -> (String, u64) {
    DISPLAY_TARGET.0.lock().map_or_else(
        |_| (ALL_DISPLAYS.to_owned(), 0),
        |state| (state.value.clone(), state.revision),
    )
}

/// 等待设置事件，并用低频超时兜底显示器热插拔与 Explorer 重建。
fn wait_for_display_change(observed_revision: u64) {
    let Ok(state) = DISPLAY_TARGET.0.lock() else {
        thread::sleep(RECOVERY_RETRY_DELAY);
        return;
    };
    if DISPLAY_TARGET
        .1
        .wait_timeout_while(state, DISPLAY_TOPOLOGY_CHECK_INTERVAL, |state| {
            state.revision == observed_revision
        })
        .is_err()
    {
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
