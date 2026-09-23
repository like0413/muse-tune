use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::media::MediaPlayer;

use super::{LyricsResponse, parse_response};
use crate::lyrics::{
    error::LyricsError,
    matcher::{SongCandidate, accepted_score},
    model::{LyricsSource, LyricsSourceKind, ResolvedLyrics},
    track::TrackDescriptor,
};

const MAX_LOCAL_LYRICS_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PLAYING_LIST_BYTES: u64 = 16 * 1024 * 1024;

/// 返回网易云 3.1.39 无扩展名歌词响应所在的临时目录。
pub(in crate::lyrics::players) fn automatic_cache_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|root| {
        PathBuf::from(root)
            .join("NetEase")
            .join("CloudMusic")
            .join("Temp")
    })
}

/// 返回最新版播放队列所在目录，与 Temp 一起通过文件事件监听。
pub(in crate::lyrics::players) fn additional_watch_path() -> Option<PathBuf> {
    automatic_cache_path()?
        .parent()
        .map(|root| root.join("webdata").join("file"))
}

/// 从播放队列定位歌曲 ID，并读取网易云当前歌曲的本地歌词响应。
pub(in crate::lyrics::players) fn resolve(
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
    let response = serde_json::from_str::<LyricsResponse>(&content)?;
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
pub(in crate::lyrics::players) fn changed_paths_affect_track(
    track: &TrackDescriptor,
    cache_path: &Path,
    paths: &[PathBuf],
    current_source: Option<&LyricsSource>,
    current_has_word_timing: bool,
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
    playing_list_changed
        && can_upgrade_from_playing_list(current_source, current_has_word_timing, &song.id)
        && cache_path.join(expected).is_file()
}

/// 播放队列变化只用于补齐或提升行级歌词，不能淘汰已经取得的逐字结果。
fn can_upgrade_from_playing_list(
    current_source: Option<&LyricsSource>,
    current_has_word_timing: bool,
    song_id: &str,
) -> bool {
    if current_has_word_timing {
        return false;
    }
    !current_source.is_some_and(|source| {
        source.player == MediaPlayer::NeteaseCloudMusic
            && source.kind == LyricsSourceKind::Local
            && source.song_id.as_deref() == Some(song_id)
    })
}

/// 网易云 3.1.39 使用歌曲 ID 的小写 MD5 作为 Temp 歌词文件名。
fn cache_file_name(song_id: &str) -> String {
    format!("{:x}", md5::compute(song_id.as_bytes()))
}

/// 从当前播放队列中选出与媒体会话最吻合的曲目。
fn find_playing_list_song(
    track: &TrackDescriptor,
    cache_path: &Path,
) -> Result<Option<CachedTrack>, LyricsError> {
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
    let playing_list = serde_json::from_slice::<PlayingList>(&fs::read(path)?)?;
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

#[derive(Deserialize)]
struct PlayingList {
    #[serde(default)]
    list: Vec<PlayingItem>,
}

#[derive(Deserialize)]
struct PlayingItem {
    track: Option<CachedTrack>,
}

#[derive(Deserialize)]
struct CachedTrack {
    id: String,
    name: String,
    duration: u64,
    #[serde(default)]
    artists: Vec<Artist>,
}

#[derive(Deserialize)]
struct Artist {
    name: String,
}
