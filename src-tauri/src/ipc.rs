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
    ///
    /// 消息接受任意可显示类型，因此命令层既能直接传入文案，也能传入内部错误本身。
    pub(crate) fn new(
        code: &'static str,
        message: impl std::fmt::Display,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            message: message.to_string(),
            retryable,
            context: None,
        }
    }
}

/// 日志与内部错误链使用的展示形式：`[命令码] 消息`。
impl std::fmt::Display for IpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "[{}] {}", self.code, self.message)
    }
}
