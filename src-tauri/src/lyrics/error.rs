/// 歌词域内部错误；适配器失败不直接等同于总体失败。
#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("读取歌词数据失败: {0}")]
    Io(#[from] std::io::Error),
    #[error("歌词数据格式无效: {0}")]
    InvalidData(String),
    #[error("歌词网络请求失败: {0}")]
    Network(#[from] reqwest::Error),
    #[error("歌词响应解析失败: {0}")]
    Json(#[from] serde_json::Error),
}
