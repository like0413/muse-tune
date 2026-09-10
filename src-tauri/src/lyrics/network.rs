use std::io::Read;

use reqwest::blocking::Response;
use serde::de::DeserializeOwned;

use super::error::LyricsError;

const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;
pub(super) const API_USER_AGENT: &str = "MuseTune/0.1";

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
