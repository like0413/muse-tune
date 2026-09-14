use std::{
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use reqwest::blocking::{RequestBuilder, Response};
use serde::de::DeserializeOwned;

use super::error::LyricsError;

const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;
pub(super) const API_USER_AGENT: &str = concat!("MuseTune/", env!("CARGO_PKG_VERSION"));
const TOTAL_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(12);
const SINGLE_REQUEST_TIMEOUT: Duration = Duration::from_secs(4);

/// 一次歌曲解析共享的总时限，避免多个串行接口分别耗尽完整超时。
#[derive(Clone)]
pub struct ResolutionDeadline {
    expires_at: Instant,
    cancelled: Arc<AtomicBool>,
}

impl ResolutionDeadline {
    /// 从真正开始处理请求时计算总时限，排队等待不会消耗预算。
    pub fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self {
            expires_at: Instant::now() + TOTAL_RESOLUTION_TIMEOUT,
            cancelled,
        }
    }

    /// 把剩余总预算应用到单个 HTTP 请求。
    pub fn apply(&self, request: RequestBuilder) -> Result<RequestBuilder, LyricsError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(LyricsError::Cancelled);
        }
        let remaining = self
            .expires_at
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(LyricsError::DeadlineExceeded)?;
        Ok(request.timeout(remaining.min(SINGLE_REQUEST_TIMEOUT)))
    }
}

/// 在反序列化前限制内部接口响应体大小，避免异常响应占用过多内存。
pub fn parse_json<T: DeserializeOwned>(response: Response) -> Result<T, LyricsError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES)
    {
        return Err(LyricsError::InvalidData(
            "歌词接口响应超过大小上限".to_owned(),
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(LyricsError::InvalidData(
            "歌词接口响应超过大小上限".to_owned(),
        ));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
