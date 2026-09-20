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

/// 标题相似度阈值；同时用于长度预筛与最终判定。
const REQUIRED_TITLE_SIMILARITY: f64 = 0.9;

/// 预先归一化的当前歌曲匹配字段，避免在逐候选打分时重复计算。
pub struct TrackMatchKey<'a> {
    track: &'a TrackDescriptor,
    normalized_title: String,
    title_char_count: usize,
}

impl<'a> TrackMatchKey<'a> {
    /// 一次算好逐候选都需要的当前歌曲归一化标题。
    pub fn new(track: &'a TrackDescriptor) -> Self {
        let normalized_title = normalize_text(&track.title);
        Self {
            title_char_count: normalized_title.chars().count(),
            normalized_title,
            track,
        }
    }

    /// 仅接受高置信度且不存在版本冲突的候选。
    pub fn score(&self, candidate: SongCandidate<'_>) -> Option<u8> {
        let track_title = &self.normalized_title;
        let candidate_title = normalize_text(candidate.title);
        if track_title.is_empty() || candidate_title.is_empty() {
            return None;
        }
        // 编辑距离不小于两串长度之差：长度差超过阈值比例时相似度必然不达标，
        // 可在进入 O(n*m) 的完整计算前淘汰绝大多数候选。
        if !could_reach_title_similarity(self.title_char_count, candidate_title.chars().count()) {
            return None;
        }
        let title_similarity = normalized_levenshtein(track_title, &candidate_title);
        // 只拒绝“候选引入了当前标题没有的版本语义”（例如当前是原版、候选是伴奏/现场版），
        // 这样既挡住误匹配到其他发行版的风险，又不会因为平台侧漏标某个标记而丢掉正确歌词。
        let track_markers = version_markers(&self.track.title);
        let candidate_markers = version_markers(candidate.title);
        if title_similarity < REQUIRED_TITLE_SIMILARITY
            || !candidate_markers.is_subset(&track_markers)
        {
            return None;
        }
        // 候选缺少当前标题的标记时（如当前是现场版、平台标题未标注），版本语义依然不同：
        // 单看标题无法与“另一个发行版”区分，但时长可以，因此后面只允许时长几乎一致。
        let version_markers_match = candidate_markers == track_markers;

        let track_artists = normalized_artist_set(&self.track.artists);
        let candidate_artists = normalized_artist_set(candidate.artists);
        let artist_score = if track_artists.is_empty() {
            // 播放器未提供艺术家（本地文件、浏览器播放等）时不能凭此直接拒绝，
            // 改由更严格的标题与时长要求承担判别。
            0
        } else if track_artists == candidate_artists {
            30
        } else if !track_artists.is_disjoint(&candidate_artists) {
            20
        } else {
            return None;
        };
        let title_score = (title_similarity * 50.0).round() as u8;
        let duration_score = match (self.track.duration_ms, candidate.duration_ms) {
            (Some(expected), Some(actual)) => {
                let difference = expected.abs_diff(actual);
                if difference <= 2_000 {
                    20
                } else if difference <= MAX_DURATION_DIFFERENCE_MS && version_markers_match {
                    // 放宽到 5 秒只适用于版本标记完全一致的候选；标记不一致时这一档被关掉。
                    15
                } else {
                    return None;
                }
            }
            (None, None) | (None, Some(_)) | (Some(_), None) => 0,
        };
        let score = title_score + artist_score + duration_score;
        // 没有艺术家分时满分只有 70，等价于要求标题几乎完全一致且时长相差不超过 2 秒。
        let required_score = if track_artists.is_empty() { 70 } else { 90 };
        let exact_without_duration = duration_score == 0
            && title_similarity == 1.0
            && !track_artists.is_empty()
            && track_artists == candidate_artists;
        (score >= required_score || exact_without_duration).then_some(score)
    }
}

/// 单次匹配的便捷入口；批量打分请复用 [`TrackMatchKey`]。
pub fn accepted_score(track: &TrackDescriptor, candidate: SongCandidate<'_>) -> Option<u8> {
    TrackMatchKey::new(track).score(candidate)
}

/// 长度预筛：只有当长度差不超过阈值比例时才可能达到所需相似度。
/// 用整数比较避免浮点边界问题，取严格大于即淘汰，与旧逻辑逐条等价。
fn could_reach_title_similarity(track_chars: usize, candidate_chars: usize) -> bool {
    let longest = track_chars.max(candidate_chars);
    track_chars.abs_diff(candidate_chars) * 10 <= longest
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
    use super::*;
    use crate::media::MediaPlayer;

    fn track(title: &str, artists: &[&str], duration_ms: Option<u64>) -> TrackDescriptor {
        TrackDescriptor {
            key: "track-key".to_owned(),
            player: MediaPlayer::QqMusic,
            title: title.to_owned(),
            artists: artists.iter().map(|value| (*value).to_owned()).collect(),
            duration_ms,
        }
    }

    /// 打分入口只关心候选的展示字段，播放器不参与匹配。
    fn score(
        track: &TrackDescriptor,
        title: &str,
        artists: &[&str],
        duration_ms: Option<u64>,
    ) -> Option<u8> {
        let artists = artists
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        accepted_score(
            track,
            SongCandidate {
                title,
                artists: &artists,
                duration_ms,
            },
        )
    }

    #[test]
    fn exact_match_scores_full_marks() {
        let current = track("夜曲", &["周杰伦"], Some(200_000));
        assert_eq!(
            score(&current, "夜曲", &["周杰伦"], Some(200_000)),
            Some(100)
        );
    }

    /// 候选引入当前标题没有的版本语义时（原版 vs 伴奏）必须拒绝，
    /// 否则会把另一发行版的歌词当成同一首歌。
    #[test]
    fn candidate_introducing_version_marker_is_rejected() {
        let current = track("夜曲", &["周杰伦"], Some(200_000));
        assert_eq!(
            score(&current, "夜曲 (伴奏)", &["周杰伦"], Some(200_000)),
            None
        );
        assert_eq!(
            score(&current, "夜曲 Live", &["周杰伦"], Some(200_000)),
            None
        );
    }

    /// 当前标题带版本标记而候选漏标时必须拒绝：两者是不同发行版，不能共用歌词。
    /// 标记文本本身就在标题里，标题相似度会随之一起下降。
    #[test]
    fn missing_version_marker_is_rejected() {
        let current = track("夜曲 (Live)", &["周杰伦"], Some(200_000));
        assert_eq!(score(&current, "夜曲", &["周杰伦"], Some(200_000)), None);
    }

    /// 5 秒内的时长放宽只适用于版本标记完全一致的候选，标记不一致时该分档被关闭。
    ///
    /// 用足够长的标题让"漏标标记"只造成轻微相似度下降，才能真正走到分档判定这一步；
    /// 短标题会先被相似度门槛拒绝，测不到这里。
    #[test]
    fn five_second_drift_requires_matching_version_markers() {
        let long_title = "a".repeat(45);
        let current = track(&format!("{long_title} Live"), &["周杰伦"], Some(200_000));

        // 标记不一致：3 秒差异落在被关闭的 15 分档上，直接拒绝。
        assert_eq!(
            score(&current, &long_title, &["周杰伦"], Some(203_000)),
            None
        );
        // 时长几乎一致：仍然接受。
        assert!(score(&current, &long_title, &["周杰伦"], Some(200_000)).is_some());
    }

    /// 版本标记完全一致时，5 秒内的时长差仍然接受。
    #[test]
    fn matching_version_markers_allow_five_second_drift() {
        let current = track("夜曲", &["周杰伦"], Some(200_000));
        assert!(score(&current, "夜曲", &["周杰伦"], Some(203_000)).is_some());
        // 超过 5 秒直接出局。
        assert_eq!(score(&current, "夜曲", &["周杰伦"], Some(210_000)), None);
    }

    #[test]
    fn disjoint_artists_are_rejected() {
        let current = track("夜曲", &["周杰伦"], Some(200_000));
        assert_eq!(score(&current, "夜曲", &["林俊杰"], Some(200_000)), None);
    }

    /// 联合歌手里有一个重合即可通过，避免平台侧漏标某位歌手就丢掉正确歌词。
    #[test]
    fn partially_overlapping_artists_are_accepted() {
        let current = track("因为爱情", &["陈奕迅", "王菲"], Some(200_000));
        assert!(score(&current, "因为爱情", &["王菲"], Some(200_000)).is_some());
    }

    /// 播放器未提供艺术家时不能凭此拒绝，改由更严格的标题与时长要求承担判别。
    #[test]
    fn missing_track_artists_raises_the_required_score() {
        let current = track("夜曲", &[], Some(200_000));
        // 标题完全一致且时长准确：刚好达到 70 分下限。
        assert_eq!(score(&current, "夜曲", &[], Some(200_000)), Some(70));
        // 缺时长时只剩标题分，达不到 70 分下限。
        assert_eq!(score(&current, "夜曲", &[], None), None);
    }

    /// 双方都没有时长时，标题与歌手完全一致仍然接受。
    #[test]
    fn exact_title_and_artists_survive_missing_duration() {
        let current = track("夜曲", &["周杰伦"], None);
        assert!(score(&current, "夜曲", &["周杰伦"], None).is_some());
    }

    #[test]
    fn length_prescreen_rejects_hopeless_candidates() {
        assert!(could_reach_title_similarity(10, 11));
        assert!(!could_reach_title_similarity(5, 12));
    }

    #[test]
    fn empty_titles_never_match() {
        assert_eq!(
            score(&track("", &["周杰伦"], None), "夜曲", &["周杰伦"], None),
            None
        );
        assert_eq!(
            score(&track("夜曲", &["周杰伦"], None), "", &["周杰伦"], None),
            None
        );
    }

    /// NFKC 归一化让全角与半角、大小写差异不会造成缓存或匹配上的重复。
    #[test]
    fn normalization_folds_width_and_case() {
        assert_eq!(normalize_text("ＡＢＣ"), normalize_text("abc"));
        assert_eq!(normalize_text("夜曲 (Live)"), "夜曲live");
        assert_eq!(normalize_text("  "), "");
    }

    /// 国内播放器常见的联合歌手分隔符都要拆分，否则艺术家集合比对会整体失配。
    #[test]
    fn artist_splitting_handles_domestic_separators() {
        assert_eq!(
            split_artists("周杰伦/费玉清"),
            vec!["周杰伦".to_owned(), "费玉清".to_owned()]
        );
        assert_eq!(
            split_artists("陈奕迅、王菲"),
            vec!["陈奕迅".to_owned(), "王菲".to_owned()]
        );
        assert!(split_artists("   ").is_empty());
    }
}
