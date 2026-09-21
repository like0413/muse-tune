//! 原生侧内部错误的统一载体：各模块用它传播失败，命令层再收敛为 `IpcError`。

/// 原生侧内部错误。
///
/// 每个变体都保持底层错误的 `Display` 文案，只负责让不同来源的失败能经由 `?`
/// 在同一条链路上传播；面向用户的分类与可重试性由命令层决定。
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// Windows Win32 / WinRT 调用失败。
    #[error("{0}")]
    Windows(#[from] windows::core::Error),
    /// WASAPI 音频采集调用失败。
    #[error("{0}")]
    Wasapi(#[from] wasapi::WasapiError),
    /// Tauri 窗口与运行时调用失败。
    #[error("{0}")]
    Tauri(#[from] tauri::Error),
    /// 线程创建等操作系统调用失败。
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// 调用方传入的参数不合法；原样重试不会成功，命令层据此判定不可重试。
    #[error("{0}")]
    InvalidInput(String),
    /// 无法归入上述分类的失败，保留原始文案。
    #[error("{0}")]
    Message(String),
}
