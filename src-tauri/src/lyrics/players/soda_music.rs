use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use flate2::read::GzDecoder;
use regex::Regex;
use reqwest::{blocking::Client, header::USER_AGENT};
use serde::Deserialize;

use crate::media::MediaPlayer;

use super::super::{
    error::LyricsError,
    matcher::{SongCandidate, accepted_score},
    model::{LyricLine, LyricWord, LyricsSource, LyricsSourceKind, ResolvedLyrics},
    network::{ResolutionDeadline, parse_json},
    track::{PlaybackWindow, TrackDescriptor},
};

const MAX_QUEUE_BYTES: u64 = 16 * 1024 * 1024;
const USER_AGENT_VALUE: &str = "Mozilla/5.0 MuseTune/0.1";
static LINE_PATTERN: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"^\[(\d+),(\d+)](.*)$"));
static WORD_PATTERN: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"<(?P<start>\d+),(?P<duration>\d+),\d+>(?P<text>[^<]*)"));

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
    let Some(song) = find_queue_song(track, cache_path)? else {
        return Ok(None);
    };
    let response = parse_json::<SodaSeoResponse>(
        deadline
            .apply(
                client
                    .get("https://beta-luna.douyin.com/luna/h5/seo_track")
                    .header(USER_AGENT, USER_AGENT_VALUE)
                    .query(&[("track_id", song.id.as_str()), ("device_platform", "web")]),
            )?
            .send()?
            .error_for_status()?,
    )?;
    let Some(content) = response.lyric.and_then(|lyric| lyric.content) else {
        return Ok(None);
    };
    let lines = parse_soda_lyrics(&content)?;
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::SodaMusic,
            kind: LyricsSourceKind::Online,
            song_id: Some(song.id),
        },
        lines,
    }))
}

/// 汽水自动路径指向包含 QueueCache 的 LunaStorage。
pub fn automatic_cache_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(|root| PathBuf::from(root).join("SodaMusic").join("LunaStorage"))
}

/// 仅当系统播放时长与汽水试听时长一致时，采用 QueueCache 声明的原曲偏移。
pub fn playback_window(
    track: &TrackDescriptor,
    cache_path: &Path,
) -> Result<Option<PlaybackWindow>, LyricsError> {
    let Some(playback_duration_ms) = track.duration_ms else {
        return Ok(None);
    };
    let Some(song) = find_queue_song(track, cache_path)? else {
        return Ok(None);
    };
    let Some(audition) = song.audition_window() else {
        return Ok(None);
    };
    if playback_duration_ms.abs_diff(audition.duration_ms) > 5_000
        || audition.start_ms == 0
        || audition.start_ms >= song.duration
    {
        return Ok(None);
    }
    Ok(Some(PlaybackWindow {
        start_ms: audition.start_ms,
        duration_ms: audition.duration_ms,
        source_duration_ms: song.duration,
    }))
}

fn find_queue_song(
    track: &TrackDescriptor,
    cache_path: &Path,
) -> Result<Option<SodaTrack>, LyricsError> {
    let queue_path = queue_cache_path(cache_path);
    let metadata = match fs::metadata(&queue_path) {
        Ok(metadata) if metadata.len() <= MAX_QUEUE_BYTES => metadata,
        Ok(_) => {
            return Err(LyricsError::InvalidData(
                "汽水队列缓存超过大小上限".to_owned(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() < 6 {
        return Ok(None);
    }
    let bytes = fs::read(queue_path)?;
    let gzip_offset = bytes
        .windows(2)
        .position(|window| window == [0x1f, 0x8b])
        .ok_or_else(|| LyricsError::InvalidData("汽水 QueueCache 缺少 gzip 数据".to_owned()))?;
    let decoder = GzDecoder::new(&bytes[gzip_offset..]);
    let mut json = String::new();
    decoder
        .take(MAX_QUEUE_BYTES + 1)
        .read_to_string(&mut json)?;
    if json.len() as u64 > MAX_QUEUE_BYTES {
        return Err(LyricsError::InvalidData(
            "汽水队列解压结果超过大小上限".to_owned(),
        ));
    }
    let feeds = serde_json::from_str::<HashMap<String, SodaFeed>>(&json)?;
    Ok(feeds
        .into_values()
        .flat_map(|feed| feed.playables)
        .filter_map(|playable| playable.track)
        .filter_map(|song| {
            let artists = song
                .artists
                .iter()
                .map(|artist| artist.name.clone())
                .collect::<Vec<_>>();
            let score = song
                .matching_durations()
                .into_iter()
                .flatten()
                .filter_map(|duration_ms| {
                    accepted_score(
                        track,
                        SongCandidate {
                            title: &song.name,
                            artists: &artists,
                            duration_ms: Some(duration_ms),
                        },
                    )
                })
                .max()?;
            Some((score, song))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, song)| song))
}

fn queue_cache_path(cache_path: &Path) -> PathBuf {
    if cache_path.join("QueueCache").is_file() {
        cache_path.join("QueueCache")
    } else {
        cache_path.join("LunaStorage").join("QueueCache")
    }
}

fn parse_soda_lyrics(content: &str) -> Result<Vec<LyricLine>, LyricsError> {
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
struct SodaFeed {
    #[serde(default)]
    playables: Vec<SodaPlayable>,
}

#[derive(Deserialize)]
struct SodaPlayable {
    track: Option<SodaTrack>,
}

#[derive(Deserialize)]
struct SodaTrack {
    id: String,
    name: String,
    duration: u64,
    #[serde(default)]
    artists: Vec<SodaArtist>,
    #[serde(default)]
    preview: Option<SodaPreview>,
    #[serde(default)]
    audition_info: Option<SodaAuditionInfo>,
    #[serde(default)]
    playable_range: Option<SodaPlayableRange>,
}

impl SodaTrack {
    fn matching_durations(&self) -> [Option<u64>; 3] {
        [
            Some(self.duration),
            self.audition_info
                .as_ref()
                .and_then(|value| value.duration_ms),
            self.preview.as_ref().and_then(|value| value.duration),
        ]
    }

    fn audition_window(&self) -> Option<SodaAuditionWindow> {
        let duration_ms = self
            .audition_info
            .as_ref()
            .and_then(|value| value.duration_ms)
            .or_else(|| self.preview.as_ref().and_then(|value| value.duration))?;
        let start_ms = self
            .audition_info
            .as_ref()
            .and_then(|value| value.start_time_ms)
            .or_else(|| self.preview.as_ref().and_then(|value| value.start))
            .or_else(|| self.playable_range.as_ref().and_then(|value| value.start))?;
        (duration_ms > 0).then_some(SodaAuditionWindow {
            start_ms,
            duration_ms,
        })
    }
}

struct SodaAuditionWindow {
    start_ms: u64,
    duration_ms: u64,
}

#[derive(Deserialize)]
struct SodaPreview {
    duration: Option<u64>,
    start: Option<u64>,
}

#[derive(Deserialize)]
struct SodaAuditionInfo {
    duration_ms: Option<u64>,
    start_time_ms: Option<u64>,
}

#[derive(Deserialize)]
struct SodaPlayableRange {
    start: Option<u64>,
}

#[derive(Deserialize)]
struct SodaArtist {
    name: String,
}

#[derive(Deserialize)]
struct SodaSeoResponse {
    lyric: Option<SodaLyric>,
}

#[derive(Deserialize)]
struct SodaLyric {
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::parse_soda_lyrics;

    #[test]
    fn parser_keeps_plain_line_when_word_tags_are_absent() {
        let lines = parse_soda_lyrics("[1000,4000]普通行歌词").expect("测试歌词应可解析");

        assert_eq!(
            lines.first().map(|line| line.text.as_str()),
            Some("普通行歌词")
        );
    }

    #[test]
    fn parser_builds_word_timing_from_tagged_line() {
        let lines =
            parse_soda_lyrics("[1000,4000]<0,500,0>逐<500,500,0>字").expect("测试歌词应可解析");

        assert_eq!(lines.first().map(|line| line.words.len()), Some(2));
    }
}
