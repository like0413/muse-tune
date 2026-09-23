use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use flate2::read::GzDecoder;
use serde::Deserialize;

use super::super::super::{error::LyricsError, model::LyricLine};

const CACHE_DIRECTORY: [&str; 3] = ["Browser", "Cache", "Cache_Data"];
const ENTRY_FILE_NAME: &str = "data_1";
const BLOCK_FILE_HEADER_BYTES: usize = 8_192;
const ENTRY_BLOCK_BYTES: usize = 256;
const ENTRY_STORE_BYTES: usize = 96;
const ENTRY_STATE_OFFSET: usize = 20;
const ENTRY_KEY_LENGTH_OFFSET: usize = 32;
const ENTRY_DATA_SIZE_OFFSET: usize = 40;
const ENTRY_DATA_ADDRESS_OFFSET: usize = 56;
const BODY_STREAM_INDEX: usize = 1;
const MAX_ENTRY_KEY_BYTES: usize = 928;
const MAX_LYRICS_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_ESTIMATED_LINE_DURATION_MS: u64 = 8_000;

/// 按完整 Track ID 定位 Chromium blockfile 条目并解析 Spotify 行级歌词。
pub(super) fn read_cached_lyrics(
    spotify_root: &Path,
    track_id: &str,
) -> Result<Option<Vec<LyricLine>>, LyricsError> {
    let cache_directory = CACHE_DIRECTORY
        .iter()
        .fold(spotify_root.to_path_buf(), |path, segment| {
            path.join(segment)
        });
    let entry_path = cache_directory.join(ENTRY_FILE_NAME);
    let entries = match fs::read(&entry_path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let Some((body_size, body_address)) = find_lyrics_entry(&entries, track_id) else {
        return Ok(None);
    };
    let body = read_data_stream(&cache_directory, body_size, body_address)?;
    let decoded = decode_body(&body)?;
    parse_response(&decoded)
}

fn find_lyrics_entry(entries: &[u8], track_id: &str) -> Option<(usize, u32)> {
    let expected_path = format!("/color-lyrics/v2/track/{track_id}");
    (BLOCK_FILE_HEADER_BYTES..entries.len())
        .step_by(ENTRY_BLOCK_BYTES)
        .filter_map(|offset| parse_entry(entries, offset))
        .find_map(|entry| {
            entry
                .key
                .contains(&expected_path)
                .then_some((entry.body_size, entry.body_address))
        })
}

fn parse_entry(entries: &[u8], offset: usize) -> Option<CacheEntry<'_>> {
    let fixed_end = offset.checked_add(ENTRY_STORE_BYTES)?;
    if fixed_end > entries.len() || read_i32(entries, offset + ENTRY_STATE_OFFSET)? != 0 {
        return None;
    }
    let key_length = usize::try_from(read_i32(entries, offset + ENTRY_KEY_LENGTH_OFFSET)?).ok()?;
    if key_length == 0 || key_length > MAX_ENTRY_KEY_BYTES {
        return None;
    }
    let key_start = offset + ENTRY_STORE_BYTES;
    let key_end = key_start.checked_add(key_length)?;
    let key = std::str::from_utf8(entries.get(key_start..key_end)?).ok()?;
    let stream_offset = BODY_STREAM_INDEX * size_of::<u32>();
    let body_size = usize::try_from(read_i32(
        entries,
        offset + ENTRY_DATA_SIZE_OFFSET + stream_offset,
    )?)
    .ok()?;
    if body_size == 0 || body_size > MAX_LYRICS_BODY_BYTES {
        return None;
    }
    let body_address = read_u32(entries, offset + ENTRY_DATA_ADDRESS_OFFSET + stream_offset)?;
    Some(CacheEntry {
        key,
        body_size,
        body_address,
    })
}

fn read_data_stream(
    cache_directory: &Path,
    size: usize,
    address: u32,
) -> Result<Vec<u8>, LyricsError> {
    if address & 0x8000_0000 == 0 {
        return Err(LyricsError::InvalidData(
            "Spotify 歌词缓存数据地址未初始化".to_owned(),
        ));
    }
    let file_type = (address >> 28) & 0x7;
    if file_type == 0 {
        let file_number = address & 0x0fff_ffff;
        return read_exact_range(
            &cache_directory.join(format!("f_{file_number:06x}")),
            0,
            size,
        );
    }
    let block_size = match file_type {
        2 => 256usize,
        3 => 1_024usize,
        4 => 4_096usize,
        _ => {
            return Err(LyricsError::InvalidData(format!(
                "Spotify 歌词缓存块类型不受支持: {file_type}"
            )));
        }
    };
    let block_count = usize::try_from((address >> 24 & 0x3) + 1).unwrap_or(1);
    if size > block_size.saturating_mul(block_count) {
        return Err(LyricsError::InvalidData(
            "Spotify 歌词缓存正文超过已分配块".to_owned(),
        ));
    }
    let file_selector = (address >> 16) & 0xff;
    let start_block = usize::try_from(address & 0xffff).unwrap_or_default();
    let offset = BLOCK_FILE_HEADER_BYTES
        .checked_add(start_block.saturating_mul(block_size))
        .ok_or_else(|| LyricsError::InvalidData("Spotify 歌词缓存偏移溢出".to_owned()))?;
    read_exact_range(
        &cache_directory.join(format!("data_{file_selector}")),
        u64::try_from(offset).unwrap_or(u64::MAX),
        size,
    )
}

fn read_exact_range(path: &Path, offset: u64, size: usize) -> Result<Vec<u8>, LyricsError> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; size];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn decode_body(body: &[u8]) -> Result<Vec<u8>, LyricsError> {
    if body.starts_with(&[0x1f, 0x8b]) {
        let mut decoded = Vec::new();
        GzDecoder::new(body)
            .take((MAX_LYRICS_BODY_BYTES + 1) as u64)
            .read_to_end(&mut decoded)?;
        if decoded.len() > MAX_LYRICS_BODY_BYTES {
            return Err(LyricsError::InvalidData(
                "Spotify 歌词缓存解压结果超过大小上限".to_owned(),
            ));
        }
        return Ok(decoded);
    }
    Ok(body.to_vec())
}

fn parse_response(body: &[u8]) -> Result<Option<Vec<LyricLine>>, LyricsError> {
    let response = serde_json::from_slice::<SpotifyLyricsResponse>(body)?;
    if !matches!(
        response.lyrics.sync_type.as_str(),
        "LINE_SYNCED" | "SYLLABLE_SYNCED"
    ) {
        return Ok(None);
    }
    let mut parsed = response
        .lyrics
        .lines
        .into_iter()
        .filter_map(|line| {
            let start_ms = line.start_time_ms.parse::<u64>().ok()?;
            let text = line.words.trim();
            (!text.is_empty()).then(|| ParsedLine {
                start_ms,
                end_ms: line.end_time_ms.parse::<u64>().unwrap_or_default(),
                text: text.to_owned(),
            })
        })
        .collect::<Vec<_>>();
    parsed.sort_unstable_by_key(|line| line.start_ms);
    let mut lines = Vec::with_capacity(parsed.len());
    let mut parsed = parsed.into_iter().peekable();
    while let Some(line) = parsed.next() {
        let fallback_end = parsed.peek().map_or_else(
            || line.start_ms.saturating_add(5_000),
            |next| next.start_ms.max(line.start_ms.saturating_add(1)),
        );
        let end_ms = if line.end_ms > line.start_ms {
            line.end_ms
        } else {
            fallback_end.min(line.start_ms.saturating_add(MAX_ESTIMATED_LINE_DURATION_MS))
        };
        lines.push(LyricLine {
            start_ms: line.start_ms,
            end_ms,
            text: line.text,
            translation: None,
            romanization: None,
            words: Vec::new(),
        });
    }
    Ok((!lines.is_empty()).then_some(lines))
}

fn read_i32(bytes: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

struct CacheEntry<'a> {
    key: &'a str,
    body_size: usize,
    body_address: u32,
}

struct ParsedLine {
    start_ms: u64,
    end_ms: u64,
    text: String,
}

#[derive(Deserialize)]
struct SpotifyLyricsResponse {
    lyrics: SpotifyLyrics,
}

#[derive(Deserialize)]
struct SpotifyLyrics {
    #[serde(rename = "syncType")]
    sync_type: String,
    #[serde(default)]
    lines: Vec<SpotifyLine>,
}

#[derive(Deserialize)]
struct SpotifyLine {
    #[serde(rename = "startTimeMs")]
    start_time_ms: String,
    #[serde(rename = "endTimeMs", default)]
    end_time_ms: String,
    words: String,
}
