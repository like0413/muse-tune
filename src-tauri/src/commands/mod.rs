pub mod data;
pub mod diagnostics;
pub mod lyrics;
pub mod media;
pub mod settings;
pub mod system;
pub mod taskbar;
pub mod tray;

use crate::{error::Error, ipc::IpcError};

/// 在阻塞线程池执行原生调用，把「等待失败」与「调用失败」统一收敛为 IPC 错误。
///
/// 等待阶段的失败只可能来自线程池，一律按可重试处理；调用阶段的失败按错误分类判定：
/// 参数非法重试也不会成功，其余同样按可重试处理。命令码与等待阶段的文案由调用点给定。
pub(crate) async fn run_blocking<T>(
    code: &'static str,
    wait_message: &str,
    task: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, IpcError>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| IpcError::new(code, format!("{wait_message}: {error}"), true))?
        .map_err(|error| match error {
            Error::InvalidInput(message) => IpcError::new(code, message, false),
            error => IpcError::new(code, error, true),
        })
}
