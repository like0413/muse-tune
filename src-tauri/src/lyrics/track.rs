use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::media::{MediaPlayer, MediaSessionSnapshot};

/// 歌词解析所需的稳定歌曲描述，不携带封面和播放状态。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackDescriptor {
    pub key: String,
    pub player: MediaPlayer,
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<u64>,
}

impl TrackDescriptor {
    /// 从 GSMTC 快照提取歌词匹配字段；空标题不能形成有效歌曲。
    pub fn from_snapshot(snapshot: &MediaSessionSnapshot) -> Option<Self> {
        let title = snapshot.metadata.title.trim();
        if title.is_empty() || snapshot.player == MediaPlayer::Other {
            return None;
        }
        let artist = [
            snapshot.metadata.artist.as_str(),
            snapshot.metadata.album_artist.as_str(),
            snapshot.metadata.subtitle.as_str(),
        ]
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default();
        let artists = split_artists(artist);
        let duration_ms = snapshot
            .timeline
            .as_ref()
            .and_then(|timeline| timeline.end_time_ms.checked_sub(timeline.start_time_ms))
            .and_then(|duration| u64::try_from(duration).ok())
            .filter(|duration| *duration > 0);
        let normalized_title = normalize_text(title);
        let mut normalized_artists = artists
            .iter()
            .map(|artist| normalize_text(artist))
            .filter(|artist| !artist.is_empty())
            .collect::<Vec<_>>();
        normalized_artists.sort_unstable();
        normalized_artists.dedup();
        let key_input = format!(
            "v1|{:?}|{}|{}|{}",
            snapshot.player,
            normalized_title,
            normalized_artists.join("/"),
            duration_ms.map_or(0, |duration| duration / 1_000)
        );
        let key = hex::encode(Sha256::digest(key_input.as_bytes()));

        Some(Self {
            key,
            player: snapshot.player,
            title: title.to_owned(),
            artists,
            duration_ms,
        })
    }
}

/// 规范化跨平台标题和歌手文本。
pub fn normalize_text(value: &str) -> String {
    value
        .nfkc()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

/// 按国内播放器常见分隔符拆分联合歌手。
pub fn split_artists(value: &str) -> Vec<String> {
    value
        .split([',', '，', '、', '/', '&', '；', ';'])
        .map(str::trim)
        .filter(|artist| !artist.is_empty())
        .map(str::to_owned)
        .collect()
}
