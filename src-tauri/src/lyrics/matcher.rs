use std::collections::BTreeSet;

use strsim::normalized_levenshtein;

use super::track::{TrackDescriptor, normalize_text, split_artists};

const VERSION_MARKERS: [(&str, &[&str]); 14] = [
    ("live", &["live", "现场"]),
    ("remix", &["remix", "混音"]),
    ("dj", &["dj", "dj版"]),
    ("instrumental", &["instrumental", "伴奏", "纯音乐"]),
    ("cover", &["cover", "翻唱"]),
    ("demo", &["demo", "小样"]),
    ("clip", &["片段", "tv size", "short version"]),
    ("acoustic", &["acoustic", "unplugged", "不插电"]),
    ("remaster", &["remaster", "remastered", "重制"]),
    ("radio_edit", &["radio edit"]),
    ("edit", &["edit", "剪辑版"]),
    ("sped_up", &["sped up", "加速版"]),
    ("slowed", &["slowed", "慢速版"]),
    ("anniversary", &["anniversary", "周年版"]),
];
pub(super) const MAX_DURATION_DIFFERENCE_MS: u64 = 5_000;

/// 可供各播放器独立适配器使用的歌曲候选元数据。
pub struct SongCandidate<'a> {
    pub title: &'a str,
    pub artists: &'a [String],
    pub duration_ms: Option<u64>,
}

/// 仅接受高置信度且不存在版本冲突的候选。
pub fn accepted_score(track: &TrackDescriptor, candidate: SongCandidate<'_>) -> Option<u8> {
    let track_title = normalize_text(&track.title);
    let candidate_title = normalize_text(candidate.title);
    if track_title.is_empty() || candidate_title.is_empty() {
        return None;
    }
    let title_similarity = normalized_levenshtein(&track_title, &candidate_title);
    if title_similarity < 0.9 || version_markers(&track.title) != version_markers(candidate.title) {
        return None;
    }

    let track_artists = normalized_artist_set(&track.artists);
    let candidate_artists = normalized_artist_set(candidate.artists);
    let artist_score = if !track_artists.is_empty() && track_artists == candidate_artists {
        30
    } else if !track_artists.is_disjoint(&candidate_artists) {
        20
    } else {
        return None;
    };
    let title_score = (title_similarity * 50.0).round() as u8;
    let duration_score = match (track.duration_ms, candidate.duration_ms) {
        (Some(expected), Some(actual)) => {
            let difference = expected.abs_diff(actual);
            if difference <= 2_000 {
                20
            } else if difference <= MAX_DURATION_DIFFERENCE_MS {
                15
            } else {
                return None;
            }
        }
        (None, None) | (None, Some(_)) | (Some(_), None) => 0,
    };
    let score = title_score + artist_score + duration_score;
    let exact_without_duration = duration_score == 0
        && title_similarity == 1.0
        && !track_artists.is_empty()
        && track_artists == candidate_artists;
    (score >= 90 || exact_without_duration).then_some(score)
}

fn normalized_artist_set(artists: &[String]) -> BTreeSet<String> {
    artists
        .iter()
        .flat_map(|artist| split_artists(artist))
        .map(|artist| normalize_text(&artist))
        .filter(|artist| !artist.is_empty())
        .collect()
}

fn version_markers(value: &str) -> BTreeSet<&'static str> {
    let normalized = value.to_lowercase();
    let ascii_words = normalized
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let padded_ascii = format!(" {ascii_words} ");
    VERSION_MARKERS
        .into_iter()
        .filter(|(_, aliases)| {
            aliases.iter().any(|alias| {
                if alias.is_ascii() {
                    padded_ascii.contains(&format!(" {alias} "))
                } else {
                    normalized.contains(alias)
                }
            })
        })
        .map(|(marker, _)| marker)
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::media::MediaPlayer;

    use super::{SongCandidate, accepted_score, version_markers};
    use crate::lyrics::track::TrackDescriptor;

    #[test]
    fn live_marker_does_not_match_inside_an_english_word() {
        assert!(version_markers("Olive Tree").is_empty());
    }

    #[test]
    fn version_markers_group_equivalent_aliases() {
        assert_eq!(
            version_markers("Song (Live)"),
            version_markers("Song 现场版")
        );
    }

    #[test]
    fn version_markers_recognize_common_new_variants() {
        let markers = version_markers("Song (Acoustic Remastered)");

        assert!(markers.contains("acoustic"));
        assert!(markers.contains("remaster"));
    }

    #[test]
    fn matcher_rejects_different_song_version() {
        let track = TrackDescriptor {
            key: "track".to_owned(),
            player: MediaPlayer::QqMusic,
            title: "歌曲 Live".to_owned(),
            artists: vec!["歌手".to_owned()],
            duration_ms: Some(180_000),
        };
        let candidate_artists = vec!["歌手".to_owned()];

        assert_eq!(
            accepted_score(
                &track,
                SongCandidate {
                    title: "歌曲",
                    artists: &candidate_artists,
                    duration_ms: Some(180_000),
                }
            ),
            None
        );
    }
}
