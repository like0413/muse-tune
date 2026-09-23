//! 汽水歌词适配器：QueueCache 只负责歌曲身份与试听判断，在线模块负责取词。

mod online;
mod queue;

use std::path::{Path, PathBuf};

use reqwest::blocking::Client;

use super::super::{
    error::LyricsError, model::ResolvedLyrics, network::ResolutionDeadline, track::TrackDescriptor,
};

pub(super) use queue::queue_contains_track;

const STANDARD_PREVIEW_DURATIONS_MS: [u64; 2] = [30_000, 60_000];
const STANDARD_PREVIEW_TOLERANCE_MS: u64 = 500;

/// 汽水通过本地 QueueCache 确认歌曲 ID，再调用无需登录的官方域名接口。
pub fn resolve(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let Some(cache_path) = cache_path else {
        return Ok(None);
    };
    let Some(song) = queue::find_song(track, cache_path)? else {
        return Ok(None);
    };
    online::resolve(song.id, client, deadline)
}

/// 汽水自动路径指向包含 QueueCache 的 LunaStorage。
pub fn automatic_cache_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(|root| PathBuf::from(root).join("SodaMusic").join("LunaStorage"))
}

/// 先识别汽水标准试听时长，其他时长再由 QueueCache 元数据补充判定。
pub fn is_preview_playback(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
) -> Result<bool, LyricsError> {
    let Some(playback_duration_ms) = track.duration_ms else {
        return Ok(false);
    };
    if matches_standard_preview_duration(playback_duration_ms) {
        return Ok(true);
    }
    let Some(cache_path) = cache_path else {
        return Ok(false);
    };
    let Some(song) = queue::find_song(track, cache_path)? else {
        return Ok(false);
    };
    let Some(audition_duration_ms) = song.audition_duration_ms() else {
        return Ok(false);
    };
    Ok(matches_preview_duration(
        playback_duration_ms,
        audition_duration_ms,
        song.duration,
    ))
}

/// 标准 30/60 秒试听在实测中有数十毫秒封装偏差，仅在窄容差内命中。
fn matches_standard_preview_duration(playback_duration_ms: u64) -> bool {
    STANDARD_PREVIEW_DURATIONS_MS
        .into_iter()
        .any(|expected| playback_duration_ms.abs_diff(expected) <= STANDARD_PREVIEW_TOLERANCE_MS)
}

/// 试听时长必须显著短于原曲，避免将带有冗余元数据的完整播放误判为试听。
fn matches_preview_duration(
    playback_duration_ms: u64,
    audition_duration_ms: u64,
    source_duration_ms: u64,
) -> bool {
    audition_duration_ms > 0
        && audition_duration_ms < source_duration_ms
        && playback_duration_ms.abs_diff(audition_duration_ms) <= 5_000
}
