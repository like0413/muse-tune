use serde::Serialize;

/// 前后端共享的结构化 IPC 错误载荷。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IpcError {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

impl IpcError {
    /// 创建不携带额外上下文的 IPC 错误。
    pub(crate) fn new(code: &'static str, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            context: None,
        }
    }
}
