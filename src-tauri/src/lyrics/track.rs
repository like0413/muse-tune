use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::media::{MediaPlayer, MediaSessionSnapshot};

use super::schema::LYRICS_CACHE_SCHEMA_VERSION;

/// 歌词解析所需的稳定歌曲描述，不携带封面和播放状态。
#[derive(Clone, Debug)]
pub struct TrackDescriptor {
    pub key: String,
    pub player: MediaPlayer,
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<u64>,
}

impl PartialEq for TrackDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for TrackDescriptor {}

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
        // 播放器和时长不参与缓存身份：前者确保跨平台复用，后者避免
        // GSMTC 的轻微时长修正生成重复条目。时长仍用于候选匹配和时间轴校验。
        let key_input = format!(
            "v{LYRICS_CACHE_SCHEMA_VERSION}|{}|{}",
            normalized_title,
            normalized_artists.join("/")
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 缓存身份由"归一化标题 + 归一化歌手"决定，因此全角、大小写与标点差异不能产生新条目，
    /// 否则同一首歌会在不同播放器或不同标题写法下反复重新解析。
    #[test]
    fn normalization_collapses_cosmetic_title_differences() {
        assert_eq!(normalize_text("夜曲"), normalize_text("夜曲"));
        assert_eq!(normalize_text("ＡＢＣ"), normalize_text("abc"));
        assert_eq!(normalize_text("夜曲 (Live)"), normalize_text("夜曲live"));
        assert_eq!(normalize_text("Hello, World!"), "helloworld");
    }

    #[test]
    fn normalization_drops_whitespace_only_titles() {
        assert!(normalize_text("   ").is_empty());
    }

    /// 联合歌手必须先拆分再归一化，否则"周杰伦/费玉清"与"周杰伦、费玉清"会得到不同身份。
    #[test]
    fn artist_splitting_is_separator_agnostic() {
        assert_eq!(
            split_artists("周杰伦/费玉清"),
            split_artists("周杰伦、费玉清")
        );
        assert_eq!(
            split_artists("周杰伦/费玉清"),
            split_artists("周杰伦，费玉清")
        );
    }
}
