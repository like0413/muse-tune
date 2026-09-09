use std::{
    fs,
    path::{Path, PathBuf},
};

use reqwest::{
    blocking::Client,
    header::{COOKIE, REFERER, USER_AGENT},
};
use serde::Deserialize;

use crate::media::MediaPlayer;

use super::super::{
    error::LyricsError,
    matcher::{SongCandidate, accepted_score},
    model::{LyricLine, LyricsSource, LyricsSourceKind, ResolvedLyrics},
    network::parse_json,
    parser::{AuxiliaryKind, merge_auxiliary_lines, parse_lrc_lines, parse_yrc_lines},
    track::{TrackDescriptor, split_artists},
};

const MAX_LOCAL_LYRICS_BYTES: u64 = 2 * 1024 * 1024;
const USER_AGENT_VALUE: &str = "MuseTune/0.1";

/// 网易云旧版本地目录只作机会式读取，未命中后使用国内 HTTPS 接口。
pub fn resolve(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
    client: &Client,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    if let Some(path) = cache_path
        && let Some(resolved) = resolve_local(track, path)?
    {
        return Ok(Some(resolved));
    }
    resolve_online(track, client)
}

/// 返回旧版网易云歌词目录；目录不存在也保留路径用于设置页诊断。
pub fn automatic_cache_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|root| {
        PathBuf::from(root)
            .join("NetEase")
            .join("CloudMusic")
            .join("webdata")
            .join("lyric")
    })
}

/// 匿名搜索网易云曲目，并优先读取响应中的 YRC 逐字歌词。
pub fn resolve_online(
    track: &TrackDescriptor,
    client: &Client,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let query = format!("{} {}", track.title, track.artists.join(" "));
    let response = parse_json::<NeteaseSearchResponse>(
        client
            .post("https://music.163.com/api/search/get")
            .header(USER_AGENT, USER_AGENT_VALUE)
            .header(REFERER, "https://music.163.com/")
            .header(COOKIE, "os=pc; appver=2.9.7; channel=netease;")
            .form(&[
                ("s", query.as_str()),
                ("type", "1"),
                ("limit", "10"),
                ("offset", "0"),
            ])
            .send()?
            .error_for_status()?,
    )?;
    let Some(song) = response.result.and_then(|result| {
        result
            .songs
            .into_iter()
            .filter_map(|song| {
                let artists = song
                    .artists
                    .iter()
                    .map(|artist| artist.name.clone())
                    .collect::<Vec<_>>();
                let score = accepted_score(
                    track,
                    SongCandidate {
                        title: &song.name,
                        artists: &artists,
                        duration_ms: Some(song.duration),
                    },
                )?;
                Some((score, song))
            })
            .max_by_key(|(score, _)| *score)
            .map(|(_, song)| song)
    }) else {
        return Ok(None);
    };

    let response = parse_json::<NeteaseLyricsResponse>(
        client
            .get("https://music.163.com/api/song/lyric")
            .header(USER_AGENT, USER_AGENT_VALUE)
            .header(REFERER, "https://music.163.com/")
            .header(COOKIE, "os=pc; appver=2.9.7; channel=netease;")
            .query(&[
                ("id", song.id.to_string()),
                ("lv", "1".to_owned()),
                ("kv", "1".to_owned()),
                ("tv", "-1".to_owned()),
                ("yv", "1".to_owned()),
            ])
            .send()?
            .error_for_status()?,
    )?;
    let lines = parse_response(&response)?;
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::NeteaseCloudMusic,
            kind: LyricsSourceKind::Online,
            song_id: Some(song.id.to_string()),
        },
        lines,
    }))
}

fn resolve_local(
    track: &TrackDescriptor,
    cache_path: &Path,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    if !cache_path.is_dir() {
        return Ok(None);
    }
    let mut best: Option<(u8, PathBuf)> = None;
    for entry in fs::read_dir(cache_path)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !matches!(
            extension.to_ascii_lowercase().as_str(),
            "lrc" | "yrc" | "json"
        ) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some((artist, title)) = stem.split_once(" - ") else {
            continue;
        };
        let artists = split_artists(artist);
        let Some(score) = accepted_score(
            track,
            SongCandidate {
                title,
                artists: &artists,
                duration_ms: None,
            },
        ) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, path));
        }
    }
    let Some((_, path)) = best else {
        return Ok(None);
    };
    if fs::metadata(&path)?.len() > MAX_LOCAL_LYRICS_BYTES {
        return Err(LyricsError::InvalidData(
            "网易云本地歌词超过大小上限".to_owned(),
        ));
    }
    let content = fs::read_to_string(&path)?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let lines = match extension.to_ascii_lowercase().as_str() {
        "yrc" => parse_yrc_lines(&content)?,
        "json" => serde_json::from_str::<NeteaseLyricsResponse>(&content)
            .map_err(LyricsError::from)
            .and_then(|response| parse_response(&response))?,
        _ => parse_lrc_lines(&content)?,
    };
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::NeteaseCloudMusic,
            kind: LyricsSourceKind::Local,
            song_id: None,
        },
        lines,
    }))
}

fn parse_response(response: &NeteaseLyricsResponse) -> Result<Vec<LyricLine>, LyricsError> {
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
struct NeteaseSearchResponse {
    result: Option<NeteaseSearchResult>,
}

#[derive(Deserialize)]
struct NeteaseSearchResult {
    #[serde(default)]
    songs: Vec<NeteaseSong>,
}

#[derive(Deserialize)]
struct NeteaseSong {
    id: u64,
    name: String,
    #[serde(default)]
    artists: Vec<NeteaseArtist>,
    #[serde(rename = "duration", alias = "dt")]
    duration: u64,
}

#[derive(Deserialize)]
struct NeteaseArtist {
    name: String,
}

#[derive(Deserialize)]
struct NeteaseLyricsResponse {
    lrc: Option<NeteaseLyricsPart>,
    #[serde(rename = "tlyric")]
    translation: Option<NeteaseLyricsPart>,
    yrc: Option<NeteaseLyricsPart>,
}

#[derive(Deserialize)]
struct NeteaseLyricsPart {
    lyric: Option<String>,
}
