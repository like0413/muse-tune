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
    model::{LyricLine, LyricsSource, LyricsSourceKind, ResolvedLyrics, has_word_timing},
    network::{API_USER_AGENT, ResolutionDeadline, parse_json},
    parser::{AuxiliaryKind, merge_auxiliary_lines, parse_lrc_lines, parse_yrc_lines},
    track::TrackDescriptor,
};

const MAX_LOCAL_LYRICS_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PLAYING_LIST_BYTES: u64 = 16 * 1024 * 1024;
const NETEASE_COOKIE: &str = "os=pc; appver=3.1.39; channel=netease;";

/// 网易云 3.1.39 本地缓存优先，未命中后使用官方域名下的网页内部 HTTPS 接口。
pub fn resolve(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let local = cache_path.and_then(|path| match resolve_local(track, path) {
        Ok(resolved) => resolved,
        Err(error) => {
            log::warn!("网易云音乐本地歌词不可用，回退在线源: {error}");
            None
        }
    });
    if local
        .as_ref()
        .is_some_and(|resolved| has_word_timing(&resolved.lines))
    {
        return Ok(local);
    }
    match resolve_online(track, client, deadline) {
        Ok(Some(online)) if has_word_timing(&online.lines) => Ok(Some(online)),
        Ok(online) => Ok(local.or(online)),
        Err(error) if local.is_some() => {
            log::warn!("网易云音乐在线逐字升级失败，保留本地逐行歌词: {error}");
            Ok(local)
        }
        Err(error) => Err(error),
    }
}

/// 返回网易云 3.1.39 无扩展名歌词响应所在的临时目录。
pub fn automatic_cache_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|root| {
        PathBuf::from(root)
            .join("NetEase")
            .join("CloudMusic")
            .join("Temp")
    })
}

/// 返回最新版播放队列所在目录，与 Temp 一起通过文件事件监听。
pub fn additional_watch_path() -> Option<PathBuf> {
    automatic_cache_path()?
        .parent()
        .map(|root| root.join("webdata").join("file"))
}

/// 匿名搜索网易云曲目，并优先读取响应中的 YRC 逐字歌词。
pub fn resolve_online(
    track: &TrackDescriptor,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let query = format!("{} {}", track.title, track.artists.join(" "));
    let response = parse_json::<NeteaseSearchResponse>(
        deadline
            .apply(
                client
                    .post("https://music.163.com/api/search/get")
                    .header(USER_AGENT, API_USER_AGENT)
                    .header(REFERER, "https://music.163.com/")
                    .header(COOKIE, NETEASE_COOKIE)
                    .form(&[
                        ("s", query.as_str()),
                        ("type", "1"),
                        ("limit", "10"),
                        ("offset", "0"),
                    ]),
            )?
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
        deadline
            .apply(
                client
                    .get("https://music.163.com/api/song/lyric")
                    .header(USER_AGENT, API_USER_AGENT)
                    .header(REFERER, "https://music.163.com/")
                    .header(COOKIE, NETEASE_COOKIE)
                    .query(&[
                        ("id", song.id.to_string()),
                        ("lv", "1".to_owned()),
                        ("kv", "1".to_owned()),
                        ("tv", "-1".to_owned()),
                        ("yv", "1".to_owned()),
                    ]),
            )?
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

pub(super) fn resolve_local(
    track: &TrackDescriptor,
    cache_path: &Path,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    if !cache_path.is_dir() {
        return Ok(None);
    }
    let Some(song) = find_playing_list_song(track, cache_path)? else {
        return Ok(None);
    };
    let path = cache_path.join(cache_file_name(&song.id));
    if !path.is_file() {
        return Ok(None);
    }
    if fs::metadata(&path)?.len() > MAX_LOCAL_LYRICS_BYTES {
        return Err(LyricsError::InvalidData(
            "网易云本地歌词超过大小上限".to_owned(),
        ));
    }
    let content = fs::read_to_string(&path)?;
    let response = serde_json::from_str::<NeteaseLyricsResponse>(&content)?;
    let lines = parse_response(&response)?;
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::NeteaseCloudMusic,
            kind: LyricsSourceKind::Local,
            song_id: Some(song.id),
        },
        lines,
    }))
}

/// 只让播放队列或当前歌曲对应的散列缓存触发网易云重新解析。
pub(super) fn changed_paths_affect_track(
    track: &TrackDescriptor,
    cache_path: &Path,
    paths: &[PathBuf],
    current_source: Option<&LyricsSource>,
) -> bool {
    let playing_list_changed = paths.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("playingList"))
    });
    let song = match find_playing_list_song(track, cache_path) {
        Ok(Some(song)) => song,
        Ok(None) => return false,
        Err(error) => {
            log::debug!("判断网易云缓存事件归属失败，按相关变化处理: {error}");
            return true;
        }
    };
    let expected = cache_file_name(&song.id);
    let lyrics_file_changed = paths.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(&expected))
    });
    if lyrics_file_changed {
        return true;
    }
    let already_uses_same_local_song = current_source.is_some_and(|source| {
        source.player == MediaPlayer::NeteaseCloudMusic
            && source.kind == LyricsSourceKind::Local
            && source.song_id.as_deref() == Some(song.id.as_str())
    });
    playing_list_changed && !already_uses_same_local_song && cache_path.join(expected).is_file()
}

/// 网易云 3.1.39 使用歌曲 ID 的小写 MD5 作为 Temp 歌词文件名。
fn cache_file_name(song_id: &str) -> String {
    format!("{:x}", md5::compute(song_id.as_bytes()))
}

fn find_playing_list_song(
    track: &TrackDescriptor,
    cache_path: &Path,
) -> Result<Option<NeteaseCachedTrack>, LyricsError> {
    let Some(root) = cache_path.parent() else {
        return Ok(None);
    };
    let path = root.join("webdata").join("file").join("playingList");
    let metadata = match fs::metadata(&path) {
        Ok(metadata) if metadata.len() <= MAX_PLAYING_LIST_BYTES => metadata,
        Ok(_) => {
            return Err(LyricsError::InvalidData(
                "网易云播放队列超过大小上限".to_owned(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() == 0 {
        return Ok(None);
    }
    let playing_list = serde_json::from_slice::<NeteasePlayingList>(&fs::read(path)?)?;
    Ok(playing_list
        .list
        .into_iter()
        .filter_map(|item| item.track)
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
        .map(|(_, song)| song))
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

#[derive(Deserialize)]
struct NeteasePlayingList {
    #[serde(default)]
    list: Vec<NeteasePlayingItem>,
}

#[derive(Deserialize)]
struct NeteasePlayingItem {
    track: Option<NeteaseCachedTrack>,
}

#[derive(Deserialize)]
struct NeteaseCachedTrack {
    id: String,
    name: String,
    duration: u64,
    #[serde(default)]
    artists: Vec<NeteaseArtist>,
}

#[cfg(test)]
mod tests {
    use super::cache_file_name;

    #[test]
    fn cache_file_name_matches_current_client_contract() {
        assert_eq!(
            cache_file_name("2612982142"),
            "a23e95de276b45a445f5fe9c87c33f6a"
        );
    }
}
