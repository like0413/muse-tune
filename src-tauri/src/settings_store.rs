use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_store::StoreExt;

use crate::storage::StoragePaths;

/// 应用内所有 Rust 模块共享的官方 Store 文件名。
pub(crate) const PATH: &str = "settings.json";

/// 禁用 GPU 加速的持久化设置键。
pub(crate) const DISABLE_GPU_ACCELERATION_KEY: &str = "app.disableGpuAcceleration";

/// 禁用 GPU 加速时注入 WebView2 的浏览器参数。
///
/// wry 默认会传 `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`，
/// 一旦使用 `additional_browser_args` 覆盖就必须自行补回这三项，故在此一并带上，再追加
/// `--disable-gpu` 关闭独立 GPU 进程（省内存，改 CPU 软渲染）。
pub(crate) const DISABLE_GPU_ACCELERATION_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu";

/// 记录启动时捕获的、本会话内所有 WebView 共用的 GPU 加速设置。
///
/// WebView2 要求同一环境内所有窗口的 `additionalBrowserArgs` 一致，否则会把 taskbar 与 settings
/// 拆成两套浏览器进程、内存反而翻倍。因此该值只在启动时读取一次，供两个窗口创建时统一使用；
/// 前端改动只写入持久化存储，重启后才被重新捕获。
#[derive(Clone, Copy)]
pub(crate) struct GpuAccelerationSetting {
    pub enabled: bool,
}

/// 若当前会话启用了禁用 GPU 加速，返回注入窗口创建器的浏览器参数；否则返回 `None`。
pub(crate) fn disable_gpu_app_args<R: Runtime>(app: &AppHandle<R>) -> Option<&'static str> {
    app.state::<GpuAccelerationSetting>()
        .enabled
        .then_some(DISABLE_GPU_ACCELERATION_ARGS)
}

/// 从持久化存储读取禁用 GPU 加速开关，供启动阶段捕获为会话内固定值。
pub(crate) fn read_disable_gpu_setting<R: Runtime>(app: &AppHandle<R>) -> GpuAccelerationSetting {
    let enabled = match app.store(app.state::<StoragePaths>().settings_file()) {
        Ok(store) => store
            .get(DISABLE_GPU_ACCELERATION_KEY)
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        Err(error) => {
            log::warn!("读取 GPU 加速设置失败，本次启动使用默认(启用加速): {error}");
            false
        }
    };
    GpuAccelerationSetting { enabled }
}
