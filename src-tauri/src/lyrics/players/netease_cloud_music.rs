//! 网易云歌词适配器：在线请求、本地缓存发现与共享响应解析按能力分层。

mod local;
mod online;

use serde::Deserialize;

use super::super::{
    error::LyricsError,
    model::LyricLine,
    parser::{AuxiliaryKind, merge_auxiliary_lines, parse_lrc_lines, parse_yrc_lines},
};

pub(super) use local::{
    additional_watch_path, automatic_cache_path, changed_paths_affect_track,
    resolve as resolve_local,
};
pub(super) use online::resolve as resolve_online;

/// 解析网易云在线接口与本地缓存共用的歌词响应。
fn parse_response(response: &LyricsResponse) -> Result<Vec<LyricLine>, LyricsError> {
    let mut lines = if let Some(lyrics) = response
        .yrc
        .as_ref()
        .and_then(|lyrics| lyrics.lyric.as_deref())
        .filter(|lyrics| !lyrics.trim().is_empty())
    {
        parse_yrc_lines(lyrics)?
    } else {
        Vec::new()
    };
    if lines.is_empty()
        && let Some(lyrics) = response
            .lrc
            .as_ref()
            .and_then(|lyrics| lyrics.lyric.as_deref())
    {
        lines = parse_lrc_lines(lyrics)?;
    }
    if let Some(translation) = response
        .translation
        .as_ref()
        .and_then(|lyrics| lyrics.lyric.as_deref())
    {
        let translation = parse_lrc_lines(translation)?;
        merge_auxiliary_lines(&mut lines, &translation, AuxiliaryKind::Translation);
    }
    Ok(lines)
}

#[derive(Deserialize)]
struct LyricsResponse {
    lrc: Option<LyricsPart>,
    #[serde(rename = "tlyric")]
    translation: Option<LyricsPart>,
    yrc: Option<LyricsPart>,
}

#[derive(Deserialize)]
struct LyricsPart {
    lyric: Option<String>,
}
