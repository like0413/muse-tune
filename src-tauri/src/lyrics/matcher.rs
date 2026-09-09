use std::collections::BTreeSet;

use strsim::normalized_levenshtein;

use super::track::{TrackDescriptor, normalize_text, split_artists};

const VERSION_MARKERS: [&str; 10] = [
    "live",
    "现场",
    "remix",
    "dj",
    "伴奏",
    "纯音乐",
    "翻唱",
    "cover",
    "demo",
    "片段",
];

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
            } else if difference <= 5_000 {
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
    VERSION_MARKERS
        .into_iter()
        .filter(|marker| normalized.contains(marker))
        .collect()
}
