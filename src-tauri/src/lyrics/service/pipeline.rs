use crate::lyrics::{
    model::{
        LyricLine, LyricsLookupMiss, LyricsLookupOutcome, LyricsResolutionMethod,
        LyricsResolutionOutcome, LyricsSnapshot, LyricsSourceKind, LyricsStatus, ResolvedLyrics,
        has_word_timing, platform_notice,
    },
    track::TrackDescriptor,
};
use crate::media::MediaPlayer;

use super::{LyricsResolutionResult, LyricsService};

/// 解析候选及其真实获取方式；候选只在流水线内流转，最终提交仍由协调器负责。
#[derive(Clone)]
pub(super) struct LyricsCandidate {
    pub(super) resolved: ResolvedLyrics,
    pub(super) resolution_method: LyricsResolutionMethod,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TimelineValidation {
    Plausible,
    DurationMismatch {
        track_duration_ms: u64,
        latest_start_ms: u64,
        latest_end_ms: u64,
    },
    /// 时间轴本身不可用。携带具体原因，避免诊断只能给出含糊的“超出歌曲有效范围”。
    Invalid(TimelineInvalidReason),
}

/// 时间轴不可用的具体原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TimelineInvalidReason {
    MissingDuration,
    EmptyLines,
    OutOfOrderLines,
    InvalidWordTiming,
}

impl TimelineInvalidReason {
    /// 诊断文案。
    pub(super) const fn detail(self) -> &'static str {
        match self {
            Self::MissingDuration => "播放器未提供有效时长",
            Self::EmptyLines => "没有可用的歌词行",
            Self::OutOfOrderLines => "歌词行时间倒退",
            Self::InvalidWordTiming => "逐字时间非法",
        }
    }
}

impl LyricsService {
    /// 保存最终歌词并仅在歌曲代数仍有效时发布。
    pub(super) fn publish_resolution(
        &self,
        track: &TrackDescriptor,
        resolved: ResolvedLyrics,
        generation: u64,
    ) {
        let resolution_method = method_from_source(&resolved);
        let snapshot = LyricsSnapshot::from_resolved(track.key.clone(), resolved);
        self.store_and_publish_if_current(snapshot, generation, resolution_method);
    }

    /// 发布已带有实际取得方式的候选，区分在线来源与应用缓存命中。
    pub(super) fn publish_candidate(
        &self,
        track: &TrackDescriptor,
        candidate: LyricsCandidate,
        generation: u64,
    ) {
        let snapshot = LyricsSnapshot::from_resolved(track.key.clone(), candidate.resolved);
        self.store_and_publish_if_current(snapshot, generation, candidate.resolution_method);
    }
}

/// 辅助内容覆盖质量；方案 A 仅在精度、当前平台和来源类型相同时参与排序。
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct AuxiliaryContentQuality {
    translation_coverage: u8,
    romanization_coverage: u8,
}

/// 跨平台候选依次比较时间精度、当前平台、本地来源和辅助内容。
pub(super) fn select_best_candidate(
    track: &TrackDescriptor,
    candidates: Vec<LyricsCandidate>,
) -> Option<LyricsCandidate> {
    candidates
        .into_iter()
        .filter(|candidate| is_acceptable_candidate(track, &candidate.resolved.lines))
        .max_by_key(|candidate| {
            (
                // 平台占位文案（“纯音乐请欣赏”“该歌曲暂无歌词”）通常只有一行且起点为 0，
                // 时间轴恒合法，若让它与真实歌词同台竞争，“当前平台”这一项就会让占位压过
                // 其他平台的真歌词，整首歌被判成纯音乐或没有歌词。因此先按“是否携带真实歌词”
                // 分层，其余维度只在同层内比较。
                u8::from(platform_notice(&candidate.resolved.lines).is_none()),
                u8::from(has_word_timing(&candidate.resolved.lines)),
                u8::from(candidate.resolved.source.player == track.player),
                u8::from(candidate.resolved.source.kind == LyricsSourceKind::Local),
                auxiliary_content_quality(&candidate.resolved.lines),
                source_priority(candidate.resolved.source.player),
            )
        })
}

/// 计算有效正文中的翻译与音译覆盖率，避免行数更多的候选天然占优。
pub(super) fn auxiliary_content_quality(lines: &[LyricLine]) -> AuxiliaryContentQuality {
    let mut eligible_count = 0usize;
    let mut translation_count = 0usize;
    let mut romanization_count = 0usize;
    for line in lines {
        let original = crate::lyrics::track::normalize_text(&line.text);
        if original.is_empty() {
            continue;
        }
        eligible_count += 1;
        translation_count += usize::from(auxiliary_text_is_meaningful(
            line.translation.as_deref(),
            &original,
        ));
        romanization_count += usize::from(auxiliary_text_is_meaningful(
            line.romanization.as_deref(),
            &original,
        ));
    }
    if eligible_count == 0 {
        return AuxiliaryContentQuality::default();
    }
    AuxiliaryContentQuality {
        translation_coverage: coverage_percent(translation_count, eligible_count),
        romanization_coverage: coverage_percent(romanization_count, eligible_count),
    }
}

/// 空辅助文本或与原文等价的占位文本不计入覆盖率。
fn auxiliary_text_is_meaningful(content: Option<&str>, original: &str) -> bool {
    content.is_some_and(|content| {
        let normalized = crate::lyrics::track::normalize_text(content);
        !normalized.is_empty() && normalized != original
    })
}

/// 将覆盖行数转换为稳定的整数百分比。
fn coverage_percent(populated: usize, eligible: usize) -> u8 {
    u8::try_from(populated.saturating_mul(100) / eligible).unwrap_or(100)
}

pub(super) fn candidate_from_source(resolved: ResolvedLyrics) -> LyricsCandidate {
    LyricsCandidate {
        resolution_method: method_from_source(&resolved),
        resolved,
    }
}

/// 只借用命中值，供文件事件比较路径使用。
pub(super) fn lookup_hit(outcome: &LyricsLookupOutcome) -> Option<&ResolvedLyrics> {
    match outcome {
        LyricsLookupOutcome::Hit(resolved) => Some(resolved),
        LyricsLookupOutcome::Unsupported | LyricsLookupOutcome::Miss(_) => None,
    }
}

pub(super) fn method_from_source(resolved: &ResolvedLyrics) -> LyricsResolutionMethod {
    match resolved.source.kind {
        LyricsSourceKind::Local => LyricsResolutionMethod::PlayerLocal,
        LyricsSourceKind::Online => LyricsResolutionMethod::Online,
    }
}

pub(super) fn summarize_resolution_result(
    result: &LyricsResolutionResult,
) -> (LyricsResolutionOutcome, Option<String>) {
    match result {
        Ok(LyricsLookupOutcome::Hit(resolved)) => {
            (LyricsResolutionOutcome::Hit, Some(source_summary(resolved)))
        }
        Ok(LyricsLookupOutcome::Miss(reason)) => (
            LyricsResolutionOutcome::Miss,
            Some(lookup_miss_detail(*reason).to_owned()),
        ),
        Ok(LyricsLookupOutcome::Unsupported) => (
            LyricsResolutionOutcome::Miss,
            Some("当前适配器不支持此解析能力".to_owned()),
        ),
        Err(error) => (LyricsResolutionOutcome::Error, Some(error.to_string())),
    }
}

/// 将稳定未命中分类转换为诊断文案，不泄漏播放器私有实现。
pub(super) fn lookup_miss_detail(reason: LyricsLookupMiss) -> &'static str {
    match reason {
        LyricsLookupMiss::DataUnavailable => "解析所需的本地数据不可用",
        LyricsLookupMiss::NoReliableLyrics => "没有找到可靠歌词",
    }
}

/// 平台展示名，用于诊断文案。
pub(super) const fn player_label(player: MediaPlayer) -> &'static str {
    match player {
        MediaPlayer::QqMusic => "QQ 音乐",
        MediaPlayer::NeteaseCloudMusic => "网易云音乐",
        MediaPlayer::SodaMusic => "汽水音乐",
        MediaPlayer::KugouMusic => "酷狗音乐",
        MediaPlayer::Other => "其他播放器",
    }
}

pub(super) fn source_summary(resolved: &ResolvedLyrics) -> String {
    let precision = if has_word_timing(&resolved.lines) {
        "逐字"
    } else {
        "逐行"
    };
    let player = player_label(resolved.source.player);
    let source_kind = match resolved.source.kind {
        LyricsSourceKind::Local => "本地",
        LyricsSourceKind::Online => "在线",
    };
    format!("{player} · {source_kind} · {precision}")
}

/// 区分播放器暂时报短的时长与歌词数据损坏：前者仍可展示，后者不可用。
pub(super) fn validate_timeline(
    track: &TrackDescriptor,
    lines: &[LyricLine],
) -> TimelineValidation {
    let Some(duration_ms) = track.duration_ms else {
        return TimelineValidation::Invalid(TimelineInvalidReason::MissingDuration);
    };
    if lines.is_empty() {
        return TimelineValidation::Invalid(TimelineInvalidReason::EmptyLines);
    }
    if lines
        .windows(2)
        .any(|pair| pair[0].start_ms > pair[1].start_ms)
    {
        return TimelineValidation::Invalid(TimelineInvalidReason::OutOfOrderLines);
    }
    let allowed_end = duration_ms.saturating_add(10_000);
    let mut latest_start_ms = 0;
    let mut latest_end_ms = 0;
    let mut exceeds_duration = false;
    for line in lines {
        latest_start_ms = latest_start_ms.max(line.start_ms);
        latest_end_ms = latest_end_ms.max(line.end_ms);
        exceeds_duration |= line.start_ms > allowed_end;
        for word in &line.words {
            if word.end_ms <= word.start_ms || word.start_ms < line.start_ms {
                return TimelineValidation::Invalid(TimelineInvalidReason::InvalidWordTiming);
            }
            latest_start_ms = latest_start_ms.max(word.start_ms);
            latest_end_ms = latest_end_ms.max(word.end_ms);
            exceeds_duration |= word.start_ms > allowed_end;
        }
    }
    if exceeds_duration {
        TimelineValidation::DurationMismatch {
            track_duration_ms: duration_ms,
            latest_start_ms,
            latest_end_ms,
        }
    } else {
        TimelineValidation::Plausible
    }
}

/// 纯音乐与“没有歌词”是无时间轴的可展示结论；普通歌词必须通过时间轴结构校验。
pub(super) fn is_cached_snapshot_displayable(
    track: &TrackDescriptor,
    snapshot: &LyricsSnapshot,
) -> bool {
    match snapshot.status {
        LyricsStatus::Instrumental | LyricsStatus::NoLyrics => true,
        LyricsStatus::Ready => is_acceptable_candidate(track, &snapshot.lines),
        LyricsStatus::Loading | LyricsStatus::Unavailable | LyricsStatus::Error => false,
    }
}

/// 判断来源结果能否参与候选排序与展示。
///
/// 播放器在切歌初期可能报出偏短的时长，这时完整歌词会被 [`validate_timeline`] 判为
/// `DurationMismatch`。这属于“时长暂时不可信”，不是数据问题：前端按播放器时间线取行，
/// 超出时长的部分自然不会显示，所以这类结果一律接受，不再限定来源——纯音乐占位文案
/// 已经在候选排序里被降到真实歌词之后（见 `select_best_candidate`）。
///
/// 只有 [`TimelineValidation::Invalid`]（缺时长、空歌词、行序倒退、逐字时间非法）才拒绝：
/// 那是数据本身不可用。
pub(super) fn is_acceptable_candidate(track: &TrackDescriptor, lines: &[LyricLine]) -> bool {
    !matches!(
        validate_timeline(track, lines),
        TimelineValidation::Invalid(_)
    )
}

/// 候选不可用时的诊断文案；可用时返回 `None`。
pub(super) fn timeline_rejection_reason(
    track: &TrackDescriptor,
    lines: &[LyricLine],
) -> Option<&'static str> {
    match validate_timeline(track, lines) {
        TimelineValidation::Invalid(reason) => Some(reason.detail()),
        TimelineValidation::Plausible | TimelineValidation::DurationMismatch { .. } => None,
    }
}

/// 以诊断友好的秒数显示毫秒时间点。
pub(super) fn format_milliseconds(milliseconds: u64) -> String {
    format!("{:.1} 秒", milliseconds as f64 / 1_000.0)
}

/// 同精度、同来源类型时保持既有的 QQ → 网易云兜底顺序。
fn source_priority(player: MediaPlayer) -> u8 {
    match player {
        MediaPlayer::QqMusic => 4,
        MediaPlayer::NeteaseCloudMusic => 3,
        MediaPlayer::SodaMusic => 2,
        MediaPlayer::KugouMusic => 1,
        MediaPlayer::Other => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::model::{LyricWord, LyricsSource, LyricsSourceKind};
    use crate::lyrics::track::TrackDescriptor;
    use crate::media::MediaPlayer;

    /// 时间轴校验与候选排序只关心播放器、时长这两个匹配字段。
    fn track(player: MediaPlayer, duration_ms: Option<u64>) -> TrackDescriptor {
        TrackDescriptor {
            key: "track-key".to_owned(),
            player,
            title: "标题".to_owned(),
            artists: vec!["歌手".to_owned()],
            duration_ms,
        }
    }

    /// 不带逐字的逐行歌词。
    fn line(start_ms: u64, end_ms: u64, text: &str) -> LyricLine {
        LyricLine {
            start_ms,
            end_ms,
            text: text.to_owned(),
            translation: None,
            romanization: None,
            words: Vec::new(),
        }
    }

    /// 单词时间与整行一致，构成完整逐字覆盖。
    fn word_line(start_ms: u64, end_ms: u64, text: &str) -> LyricLine {
        LyricLine {
            words: vec![LyricWord {
                start_ms,
                end_ms,
                text: text.to_owned(),
            }],
            ..line(start_ms, end_ms, text)
        }
    }

    fn candidate(
        player: MediaPlayer,
        kind: LyricsSourceKind,
        lines: Vec<LyricLine>,
    ) -> LyricsCandidate {
        candidate_from_source(ResolvedLyrics {
            source: LyricsSource {
                player,
                kind,
                song_id: None,
            },
            lines,
        })
    }

    fn with_translation(mut target: LyricLine, translation: &str) -> LyricLine {
        target.translation = Some(translation.to_owned());
        target
    }

    #[test]
    fn validate_timeline_rejects_missing_duration() {
        assert_eq!(
            validate_timeline(&track(MediaPlayer::QqMusic, None), &[line(0, 1_000, "a")]),
            TimelineValidation::Invalid(TimelineInvalidReason::MissingDuration)
        );
    }

    #[test]
    fn validate_timeline_rejects_empty_lines() {
        assert_eq!(
            validate_timeline(&track(MediaPlayer::QqMusic, Some(1_000)), &[]),
            TimelineValidation::Invalid(TimelineInvalidReason::EmptyLines)
        );
    }

    #[test]
    fn validate_timeline_rejects_out_of_order_lines() {
        assert_eq!(
            validate_timeline(
                &track(MediaPlayer::QqMusic, Some(10_000)),
                &[line(3_000, 4_000, "a"), line(1_000, 2_000, "b")],
            ),
            TimelineValidation::Invalid(TimelineInvalidReason::OutOfOrderLines)
        );
    }

    /// 逐字区间反向或早于整行起点都会让前端无法定位高亮位置，必须判为数据不可用。
    #[test]
    fn validate_timeline_rejects_invalid_word_timing() {
        let reversed = LyricLine {
            words: vec![LyricWord {
                start_ms: 900,
                end_ms: 900,
                text: "a".to_owned(),
            }],
            ..line(0, 1_000, "a")
        };
        assert_eq!(
            validate_timeline(&track(MediaPlayer::QqMusic, Some(10_000)), &[reversed]),
            TimelineValidation::Invalid(TimelineInvalidReason::InvalidWordTiming)
        );

        let before_line_start = LyricLine {
            words: vec![LyricWord {
                start_ms: 10,
                end_ms: 500,
                text: "a".to_owned(),
            }],
            ..line(500, 1_000, "a")
        };
        assert_eq!(
            validate_timeline(
                &track(MediaPlayer::QqMusic, Some(10_000)),
                &[before_line_start]
            ),
            TimelineValidation::Invalid(TimelineInvalidReason::InvalidWordTiming)
        );
    }

    /// 播放器在切歌初期常报偏短时长，10 秒容差内的超出必须仍然视为可用。
    #[test]
    fn validate_timeline_tolerates_overshoot_within_slack() {
        assert_eq!(
            validate_timeline(
                &track(MediaPlayer::QqMusic, Some(60_000)),
                &[line(0, 1_000, "a"), line(69_000, 70_000, "b")],
            ),
            TimelineValidation::Plausible
        );
    }

    /// 超出容差时要带出实际越界位置，诊断才能区分"报短"与"数据错"。
    #[test]
    fn validate_timeline_reports_duration_mismatch_beyond_slack() {
        let validation = validate_timeline(
            &track(MediaPlayer::QqMusic, Some(60_000)),
            &[line(0, 1_000, "a"), line(80_000, 81_000, "b")],
        );
        assert_eq!(
            validation,
            TimelineValidation::DurationMismatch {
                track_duration_ms: 60_000,
                latest_start_ms: 80_000,
                latest_end_ms: 81_000,
            }
        );
    }

    /// 逐字时间也要参与越界判定，否则只有单词超界的歌词会被误判为可用。
    #[test]
    fn validate_timeline_counts_word_timing_towards_bounds() {
        let over = LyricLine {
            words: vec![LyricWord {
                start_ms: 90_000,
                end_ms: 91_000,
                text: "a".to_owned(),
            }],
            ..line(0, 1_000, "a")
        };
        assert!(matches!(
            validate_timeline(&track(MediaPlayer::QqMusic, Some(60_000)), &[over]),
            TimelineValidation::DurationMismatch {
                latest_start_ms: 90_000,
                latest_end_ms: 91_000,
                ..
            }
        ));
    }

    /// 时长不匹配属于"时长暂时不可信"而非数据损坏，不能因此丢掉整首歌的歌词。
    #[test]
    fn duration_mismatch_is_still_acceptable() {
        let mismatched = [line(0, 1_000, "a"), line(80_000, 81_000, "b")];
        assert!(is_acceptable_candidate(
            &track(MediaPlayer::QqMusic, Some(60_000)),
            &mismatched
        ));
    }

    #[test]
    fn invalid_timeline_is_not_acceptable() {
        assert!(!is_acceptable_candidate(
            &track(MediaPlayer::QqMusic, None),
            &[line(0, 1_000, "a")]
        ));
        assert!(!is_acceptable_candidate(
            &track(MediaPlayer::QqMusic, Some(10_000)),
            &[]
        ));
    }

    /// 诊断文案只在数据真的不可用时给出，轻微时长偏差不应产生误报。
    #[test]
    fn rejection_reason_only_reported_for_invalid_data() {
        let track = track(MediaPlayer::QqMusic, Some(60_000));
        assert_eq!(
            timeline_rejection_reason(&track, &[line(0, 1_000, "a"), line(80_000, 81_000, "b")]),
            None
        );
        assert_eq!(
            timeline_rejection_reason(&track, &[]),
            Some(TimelineInvalidReason::EmptyLines.detail())
        );
    }

    /// 平台占位文案（"纯音乐请欣赏"）时间轴恒合法，若按"当前平台优先"参与排序会压过
    /// 其他平台的真歌词，把整首歌判成纯音乐。真实歌词必须优先。
    #[test]
    fn real_lyrics_beat_platform_notice() {
        let notice = candidate(
            MediaPlayer::QqMusic,
            LyricsSourceKind::Online,
            vec![line(0, 5_000, "纯音乐，请您欣赏")],
        );
        let real = candidate(
            MediaPlayer::KugouMusic,
            LyricsSourceKind::Online,
            vec![line(0, 5_000, "第一句"), line(5_000, 10_000, "第二句")],
        );
        let selected = select_best_candidate(
            &track(MediaPlayer::QqMusic, Some(200_000)),
            vec![notice, real],
        )
        .expect("真歌词应当被选中");
        assert_eq!(selected.resolved.source.player, MediaPlayer::KugouMusic);
    }

    #[test]
    fn word_timing_beats_line_precision() {
        let lined = candidate(
            MediaPlayer::QqMusic,
            LyricsSourceKind::Local,
            vec![line(0, 5_000, "第一句"), line(5_000, 10_000, "第二句")],
        );
        let worded = candidate(
            MediaPlayer::KugouMusic,
            LyricsSourceKind::Online,
            vec![
                word_line(0, 5_000, "第一句"),
                word_line(5_000, 10_000, "第二句"),
            ],
        );
        let selected = select_best_candidate(
            &track(MediaPlayer::QqMusic, Some(200_000)),
            vec![lined, worded],
        )
        .expect("逐字歌词应当胜出");
        assert_eq!(selected.resolved.source.player, MediaPlayer::KugouMusic);
    }

    /// 同精度层内，当前播放器的来源优于其他平台——这是"当前平台优先"策略的落点。
    #[test]
    fn current_player_wins_within_same_layer() {
        let other_player = candidate(
            MediaPlayer::KugouMusic,
            LyricsSourceKind::Online,
            vec![line(0, 5_000, "第一句"), line(5_000, 10_000, "第二句")],
        );
        let current_player = candidate(
            MediaPlayer::QqMusic,
            LyricsSourceKind::Online,
            vec![line(0, 5_000, "甲"), line(5_000, 10_000, "乙")],
        );
        let selected = select_best_candidate(
            &track(MediaPlayer::QqMusic, Some(200_000)),
            vec![other_player, current_player],
        )
        .expect("应当有候选胜出");
        assert_eq!(selected.resolved.source.player, MediaPlayer::QqMusic);
    }

    /// 平台、精度、来源类型都相同时，辅助内容覆盖率打破平局。
    #[test]
    fn auxiliary_coverage_breaks_ties() {
        let bare = candidate(
            MediaPlayer::QqMusic,
            LyricsSourceKind::Online,
            vec![line(0, 5_000, "第一句"), line(5_000, 10_000, "第二句")],
        );
        let translated = candidate(
            MediaPlayer::QqMusic,
            LyricsSourceKind::Online,
            vec![
                with_translation(line(0, 5_000, "第一句"), "first"),
                with_translation(line(5_000, 10_000, "第二句"), "second"),
            ],
        );
        let selected = select_best_candidate(
            &track(MediaPlayer::QqMusic, Some(200_000)),
            vec![bare, translated],
        )
        .expect("应当有候选胜出");
        assert!(selected.resolved.lines[0].translation.is_some());
    }

    #[test]
    fn unacceptable_candidates_are_dropped() {
        let no_duration = candidate(
            MediaPlayer::QqMusic,
            LyricsSourceKind::Online,
            vec![line(0, 5_000, "第一句")],
        );
        assert!(
            select_best_candidate(&track(MediaPlayer::QqMusic, None), vec![no_duration]).is_none()
        );
    }

    #[test]
    fn empty_candidates_yield_none() {
        assert!(
            select_best_candidate(&track(MediaPlayer::QqMusic, Some(200_000)), Vec::new())
                .is_none()
        );
    }

    /// 空文本与"与原文等价"的占位译文都不算有效辅助内容，否则覆盖率会被虚高抬高。
    #[test]
    fn auxiliary_quality_ignores_empty_and_duplicate_text() {
        let quality = auxiliary_content_quality(&[
            with_translation(line(0, 1_000, "第一句"), "first"),
            with_translation(line(1_000, 2_000, "第二句"), "第二句"),
            with_translation(line(2_000, 3_000, "第三句"), "   "),
        ]);
        assert_eq!(
            quality,
            AuxiliaryContentQuality {
                translation_coverage: 33,
                romanization_coverage: 0,
            }
        );
    }

    /// 纯音乐与"没有歌词"不携带歌词行，仍然是可以展示的结论。
    #[test]
    fn cached_semantic_states_are_displayable_without_timeline() {
        let track = track(MediaPlayer::QqMusic, Some(200_000));
        for status in [LyricsStatus::Instrumental, LyricsStatus::NoLyrics] {
            let snapshot = LyricsSnapshot {
                status,
                ..LyricsSnapshot::default()
            };
            assert!(is_cached_snapshot_displayable(&track, &snapshot));
        }
    }

    #[test]
    fn cached_loading_and_error_are_not_displayable() {
        let track = track(MediaPlayer::QqMusic, Some(200_000));
        for status in [
            LyricsStatus::Loading,
            LyricsStatus::Unavailable,
            LyricsStatus::Error,
        ] {
            let snapshot = LyricsSnapshot {
                status,
                ..LyricsSnapshot::default()
            };
            assert!(!is_cached_snapshot_displayable(&track, &snapshot));
        }
    }

    #[test]
    fn source_summary_distinguishes_precision_and_origin() {
        let local_word = ResolvedLyrics {
            source: LyricsSource {
                player: MediaPlayer::KugouMusic,
                kind: LyricsSourceKind::Local,
                song_id: None,
            },
            lines: vec![word_line(0, 1_000, "a")],
        };
        assert_eq!(source_summary(&local_word), "酷狗音乐 · 本地 · 逐字");
    }
}
