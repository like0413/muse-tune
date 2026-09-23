use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    lyrics::{
        error::LyricsError,
        model::{LyricsSource, LyricsSourceKind, ResolvedLyrics},
        track::TrackDescriptor,
    },
    media::MediaPlayer,
};

use super::{cache, state};

/// 从 Spotify 当前轨道状态取得精确 Track ID，再读取同一 ID 的本地歌词响应缓存。
pub(in crate::lyrics::players) fn resolve(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let Some(cache_path) = cache_path else {
        return Ok(None);
    };
    let Some(identity) = state::find_current_track(cache_path, track)? else {
        return Ok(None);
    };
    let Some(lines) = cache::read_cached_lyrics(cache_path, &identity.id)? else {
        return Ok(None);
    };
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::Spotify,
            kind: LyricsSourceKind::Local,
            song_id: Some(identity.id),
        },
        lines,
    }))
}

/// Spotify 的本地数据根目录同时包含播放状态与 Chromium HTTP 缓存。
pub(in crate::lyrics::players) fn automatic_cache_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|root| root.join("Spotify"))
        .filter(|path| path.is_dir())
}

/// 返回歌词响应目录以及每个登录账户的播放状态目录。
pub(in crate::lyrics::players) fn additional_watch_paths() -> Vec<PathBuf> {
    let Some(root) = automatic_cache_path() else {
        return Vec::new();
    };
    let mut paths = vec![root.join("Browser").join("Cache").join("Cache_Data")];
    let users = root.join("Users");
    paths.push(users.clone());
    let Ok(entries) = fs::read_dir(&users) else {
        return paths;
    };
    paths.extend(entries.filter_map(Result::ok).filter_map(|entry| {
        entry
            .file_type()
            .ok()
            .filter(|kind| kind.is_dir())
            .and_then(|_| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with("-user"))
                    .then(|| entry.path())
            })
    }));
    paths
}

/// 只在当前歌曲尚未使用同一 Spotify ID、且本地缓存已完整落盘时触发重解析。
pub(in crate::lyrics::players) fn changed_paths_affect_track(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
    paths: &[PathBuf],
    current_source: Option<&LyricsSource>,
) -> bool {
    if !paths.iter().any(|path| is_relevant_cache_file(path)) {
        return false;
    }
    let Some(cache_path) = cache_path else {
        return false;
    };
    let identity = match state::find_current_track(cache_path, track) {
        Ok(Some(identity)) => identity,
        Ok(None) => return false,
        Err(error) => {
            log::debug!("读取 Spotify 当前轨道状态失败: {error}");
            return false;
        }
    };
    if current_source.is_some_and(|source| {
        source.player == MediaPlayer::Spotify
            && source.kind == LyricsSourceKind::Local
            && source.song_id.as_deref() == Some(identity.id.as_str())
    }) {
        return false;
    }
    match cache::read_cached_lyrics(cache_path, &identity.id) {
        Ok(Some(lines)) => !lines.is_empty(),
        Ok(None) => false,
        Err(error) => {
            log::debug!("检查 Spotify 歌词缓存变化失败: {error}");
            false
        }
    }
}

fn is_relevant_cache_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("context_player_state_restore")
                || matches!(name, "data_1" | "data_2" | "data_3")
                || name.starts_with("f_")
        })
}
