use std::sync::LazyLock;

use regex::Regex;
use reqwest::{blocking::Client, header::USER_AGENT};
use serde::Deserialize;

use crate::{
    lyrics::{
        error::LyricsError,
        model::{LyricLine, LyricWord, LyricsSource, LyricsSourceKind, ResolvedLyrics},
        network::{ResolutionDeadline, parse_json},
    },
    media::MediaPlayer,
};

const USER_AGENT_VALUE: &str = concat!("Mozilla/5.0 MuseTune/", env!("CARGO_PKG_VERSION"));
static LINE_PATTERN: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"^\[(\d+),(\d+)](.*)$"));
static WORD_PATTERN: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"<(?P<start>\d+),(?P<duration>\d+),\d+>(?P<text>[^<]*)"));

/// 使用汽水歌曲 ID 从公开页面接口读取逐字歌词。
pub(super) fn resolve(
    song_id: String,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let response = parse_json::<SeoResponse>(
        deadline
            .apply(
                client
                    .get("https://beta-luna.douyin.com/luna/h5/seo_track")
                    .header(USER_AGENT, USER_AGENT_VALUE)
                    .query(&[("track_id", song_id.as_str()), ("device_platform", "web")]),
            )?
            .send()?
            .error_for_status()?,
    )?;
    let Some(content) = response.lyric.and_then(|lyric| lyric.content) else {
        return Ok(None);
    };
    let lines = parse_lyrics(&content)?;
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::SodaMusic,
            kind: LyricsSourceKind::Online,
            song_id: Some(song_id),
        },
        lines,
    }))
}

/// 解析汽水的行时间与逐字时间组合格式。
fn parse_lyrics(content: &str) -> Result<Vec<LyricLine>, LyricsError> {
    let line_pattern = LINE_PATTERN
        .as_ref()
        .map_err(|error| LyricsError::InvalidData(format!("汽水歌词行规则无效: {error}")))?;
    let word_pattern = WORD_PATTERN
        .as_ref()
        .map_err(|error| LyricsError::InvalidData(format!("汽水逐字规则无效: {error}")))?;
    let mut lines = Vec::new();
    for raw_line in content.lines() {
        let Some(captures) = line_pattern.captures(raw_line.trim()) else {
            continue;
        };
        let Some(start_ms) = captures
            .get(1)
            .and_then(|value| value.as_str().parse::<u64>().ok())
        else {
            continue;
        };
        let Some(duration_ms) = captures
            .get(2)
            .and_then(|value| value.as_str().parse::<u64>().ok())
        else {
            continue;
        };
        let body = captures.get(3).map_or("", |value| value.as_str());
        let words = word_pattern
            .captures_iter(body)
            .filter_map(|word| {
                let relative_start = word.name("start")?.as_str().parse::<u64>().ok()?;
                let duration = word.name("duration")?.as_str().parse::<u64>().ok()?;
                let text = word.name("text")?.as_str();
                let word_start_ms = start_ms.saturating_add(relative_start);
                (!text.is_empty() && duration > 0).then(|| LyricWord {
                    start_ms: word_start_ms,
                    end_ms: word_start_ms.saturating_add(duration),
                    text: text.to_owned(),
                })
            })
            .collect::<Vec<_>>();
        let text = if words.is_empty() {
            body.trim().to_owned()
        } else {
            words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<String>()
        };
        if text.trim().is_empty() {
            continue;
        }
        lines.push(LyricLine {
            start_ms,
            end_ms: start_ms.saturating_add(duration_ms),
            text,
            translation: None,
            romanization: None,
            words,
        });
    }
    lines.sort_unstable_by_key(|line| line.start_ms);
    Ok(lines)
}

#[derive(Deserialize)]
struct SeoResponse {
    lyric: Option<Lyric>,
}

#[derive(Deserialize)]
struct Lyric {
    content: Option<String>,
}
