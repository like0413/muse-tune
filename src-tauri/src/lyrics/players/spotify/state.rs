use std::{collections::HashMap, fs, path::Path, time::UNIX_EPOCH};

use prost::Message;

use super::super::super::{error::LyricsError, track::TrackDescriptor, track::normalize_text};

const STATE_FILE_NAME: &str = "context_player_state_restore";
const TRACK_URI_PREFIX: &str = "spotify:track:";
const TRACK_ID_LENGTH: usize = 22;

pub(super) struct SpotifyTrackIdentity {
    pub(super) id: String,
}

/// 从最近更新的账户状态中读取当前轨道，并用 GSMTC 标题阻止切歌竞态误配。
pub(super) fn find_current_track(
    spotify_root: &Path,
    track: &TrackDescriptor,
) -> Result<Option<SpotifyTrackIdentity>, LyricsError> {
    let users_path = spotify_root.join("Users");
    let entries = match fs::read_dir(users_path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut state_files = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_dir())
                .and_then(|_| {
                    entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.ends_with("-user"))
                        .then(|| entry.path().join(STATE_FILE_NAME))
                })
        })
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    state_files.sort_unstable_by_key(|path| {
        std::cmp::Reverse(
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH),
        )
    });

    let expected_title = normalize_text(&track.title);
    for path in state_files {
        match read_current_track(&path) {
            Ok(Some(current)) if normalize_text(&current.title) == expected_title => {
                return Ok(Some(SpotifyTrackIdentity { id: current.id }));
            }
            Ok(_) => {}
            Err(error) => log::debug!("解析 Spotify 状态文件 {} 失败: {error}", path.display()),
        }
    }
    Ok(None)
}

fn read_current_track(path: &Path) -> Result<Option<CurrentTrack>, LyricsError> {
    let bytes = fs::read(path)?;
    let Some(separator) = bytes.iter().position(|byte| *byte == b'#') else {
        return Err(LyricsError::InvalidData(
            "Spotify 状态缺少时间戳分隔符".to_owned(),
        ));
    };
    if !(10..=20).contains(&separator) || !bytes[..separator].iter().all(u8::is_ascii_digit) {
        return Err(LyricsError::InvalidData(
            "Spotify 状态时间戳无效".to_owned(),
        ));
    }
    let root = RestoreRoot::decode(&bytes[separator + 1..]).map_err(|error| {
        LyricsError::InvalidData(format!("Spotify 当前轨道 protobuf 无法解析: {error}"))
    })?;
    let Some(provided) = root
        .container
        .and_then(|value| value.container)
        .and_then(|value| value.player_state)
        .and_then(|value| value.current_track)
    else {
        return Ok(None);
    };
    let Some(id) = provided
        .uri
        .strip_prefix(TRACK_URI_PREFIX)
        .filter(|id| is_track_id(id))
        .map(str::to_owned)
    else {
        return Ok(None);
    };
    let Some(title) = provided
        .metadata
        .get("title")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
    else {
        return Ok(None);
    };
    Ok(Some(CurrentTrack { id, title }))
}

fn is_track_id(value: &str) -> bool {
    value.len() == TRACK_ID_LENGTH && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

struct CurrentTrack {
    id: String,
    title: String,
}

// Spotify 没有公开该恢复文件的完整 schema；这里只声明实机验证过的当前轨道链路，
// 其余字段交给 protobuf 的未知字段兼容机制跳过，避免扫描历史轨道 URI 造成误配。
#[derive(Clone, PartialEq, Message)]
struct RestoreRoot {
    #[prost(message, optional, tag = "3")]
    container: Option<RestoreContainer>,
}

#[derive(Clone, PartialEq, Message)]
struct RestoreContainer {
    #[prost(message, optional, tag = "3")]
    container: Option<PlayerStateContainer>,
}

#[derive(Clone, PartialEq, Message)]
struct PlayerStateContainer {
    #[prost(message, optional, tag = "1")]
    player_state: Option<PlayerState>,
}

#[derive(Clone, PartialEq, Message)]
struct PlayerState {
    #[prost(message, optional, tag = "3")]
    current_track: Option<ProvidedTrack>,
}

#[derive(Clone, PartialEq, Message)]
struct ProvidedTrack {
    #[prost(string, tag = "2")]
    uri: String,
    #[prost(map = "string, string", tag = "3")]
    metadata: HashMap<String, String>,
}
