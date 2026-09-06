//! 使用 Win32 所有者窗口关系，将 Tauri bar 窗口集成到 Windows 任务栏。

mod elements;
mod events;
mod geometry;
mod layout;
mod platform;
mod sync;

use std::{thread, time::Duration};

use tauri::{AppHandle, Manager, Runtime};

pub use geometry::TaskbarPlacement;
const TASKBAR_WINDOW_LABEL: &str = "taskbar";
const RECOVERY_RETRY_DELAY: Duration = Duration::from_millis(400);

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

/// 启动独立监控线程，持续维护任务栏与播放器窗口的所有者关系。
pub fn initialize<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    thread::Builder::new()
        .name("taskbar-monitor".to_owned())
        .spawn(move || maintain_bar_window(app_handle))?;

    Ok(())
}

/// 获取或重建播放器窗口，并在 Explorer 生命周期内持续恢复同步。
fn maintain_bar_window<R: Runtime>(app: AppHandle<R>) {
    loop {
        let Some(config) = app
            .config()
            .app
            .windows
            .iter()
            .find(|config| config.label == TASKBAR_WINDOW_LABEL)
            .cloned()
        else {
            return;
        };
        let content_width_dip = config.width.round().clamp(1.0, f64::from(i32::MAX)) as i32;
        let bar_window = if let Some(bar_window) = app.get_webview_window(TASKBAR_WINDOW_LABEL) {
            bar_window
        } else {
            let Ok(builder) = tauri::WebviewWindowBuilder::from_config(&app, &config) else {
                wait_before_recovery();
                continue;
            };
            let Ok(bar_window) = builder.build() else {
                wait_before_recovery();
                continue;
            };
            bar_window
        };

        let Ok(window_handle) = bar_window.hwnd() else {
            wait_before_recovery();
            continue;
        };
        sync::run(window_handle.0 as isize, content_width_dip);
        wait_before_recovery();
    }
}

/// 在 Explorer 或窗口短暂不可用时限制重试频率。
fn wait_before_recovery() {
    thread::sleep(RECOVERY_RETRY_DELAY);
}
