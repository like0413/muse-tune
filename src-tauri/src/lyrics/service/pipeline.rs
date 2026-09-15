use super::{
    LabeledLyricsResolutionResult, LyricsError, LyricsResolutionResult, LyricsService, MediaPlayer,
    ResolvedLyrics, TrackDescriptor,
};
use crate::lyrics::model::{
    LyricLine, LyricsLookupMiss, LyricsLookupOutcome, LyricsResolutionMethod,
    LyricsResolutionOutcome, LyricsSnapshot, LyricsSourceKind, LyricsStatus, has_word_timing,
};

/// 解析候选及其真实获取方式；候选只在流水线内流转，最终提交仍由协调器负责。
pub(super) struct LyricsCandidate {
    pub(super) resolved: ResolvedLyrics,
    pub(super) resolution_method: LyricsResolutionMethod,
}

#[derive(Clone, Copy)]
pub(super) enum TimelineValidation {
    Plausible,
    DurationMismatch {
        track_duration_ms: u64,
        latest_start_ms: u64,
        latest_end_ms: u64,
    },
    Invalid,
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

    /// 汇总同一在线阶段的结果，统一写入诊断链路与候选集合。
    pub(super) fn collect_online_results<const N: usize>(
        &self,
        generation: u64,
        parallel_group: Option<&str>,
        results: [LabeledLyricsResolutionResult<'_>; N],
        candidates: &mut Vec<LyricsCandidate>,
    ) {
        for (label, result) in results {
            let Some(result) = result else {
                continue;
            };
            let (outcome, detail) = summarize_resolution_result(&result);
            self.record_resolution_step_in_group(
                generation,
                parallel_group,
                label,
                outcome,
                detail,
            );
            match result {
                Ok(LyricsLookupOutcome::Hit(resolved)) => {
                    candidates.push(candidate_from_source(resolved));
                }
                Ok(LyricsLookupOutcome::Unsupported | LyricsLookupOutcome::Miss(_))
                | Err(LyricsError::Cancelled) => {}
                Err(error) => log::warn!("跨平台歌词适配器失败: {error}"),
            }
        }
    }
}

/// 跨平台候选先比较时间精度，再保持当前平台和本地来源优先。
pub(super) fn select_best_candidate(
    track: &TrackDescriptor,
    candidates: Vec<LyricsCandidate>,
) -> Option<LyricsCandidate> {
    candidates
        .into_iter()
        .filter(|candidate| is_plausible_timeline(track, &candidate.resolved.lines))
        .max_by_key(|candidate| {
            (
                u8::from(has_word_timing(&candidate.resolved.lines)),
                u8::from(candidate.resolved.source.player == track.player),
                u8::from(candidate.resolved.source.kind == LyricsSourceKind::Local),
                source_priority(candidate.resolved.source.player),
            )
        })
}

/// 统计翻译和音译覆盖量，用于识别同精度歌词中的内容增强结果。
pub(super) fn auxiliary_content_count(lines: &[LyricLine]) -> usize {
    lines
        .iter()
        .map(|line| {
            usize::from(line.translation.is_some()) + usize::from(line.romanization.is_some())
        })
        .sum()
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

pub(super) fn source_summary(resolved: &ResolvedLyrics) -> String {
    let precision = if has_word_timing(&resolved.lines) {
        "逐字"
    } else {
        "逐行"
    };
    let player = match resolved.source.player {
        MediaPlayer::QqMusic => "QQ 音乐",
        MediaPlayer::NeteaseCloudMusic => "网易云音乐",
        MediaPlayer::SodaMusic => "汽水音乐",
        MediaPlayer::KugouMusic => "酷狗音乐",
        MediaPlayer::Other => "其他播放器",
    };
    let source_kind = match resolved.source.kind {
        LyricsSourceKind::Local => "本地",
        LyricsSourceKind::Online => "在线",
    };
    format!("{player} · {source_kind} · {precision}")
}

/// 区分播放器暂时报短的时长与歌词数据损坏，供缓存展示和新候选校验采用不同策略。
pub(super) fn validate_timeline(
    track: &TrackDescriptor,
    lines: &[LyricLine],
) -> TimelineValidation {
    let Some(duration_ms) = track.duration_ms else {
        return TimelineValidation::Invalid;
    };
    if lines.is_empty()
        || lines
            .windows(2)
            .any(|pair| pair[0].start_ms > pair[1].start_ms)
    {
        return TimelineValidation::Invalid;
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
                return TimelineValidation::Invalid;
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

/// 已持久化的缓存只拒绝结构损坏，播放器暂时报短时长不影响立即展示。
fn is_cached_timeline_displayable(validation: TimelineValidation) -> bool {
    matches!(
        validation,
        TimelineValidation::Plausible | TimelineValidation::DurationMismatch { .. }
    )
}

/// 纯音乐是无时间轴的可展示结论；普通歌词仍必须通过缓存时间轴结构校验。
pub(super) fn is_cached_snapshot_displayable(
    track: &TrackDescriptor,
    snapshot: &LyricsSnapshot,
) -> bool {
    match snapshot.status {
        LyricsStatus::Instrumental => true,
        LyricsStatus::Ready => {
            is_cached_timeline_displayable(validate_timeline(track, &snapshot.lines))
        }
        LyricsStatus::Loading | LyricsStatus::Unavailable | LyricsStatus::Error => false,
    }
}

/// 拒绝明显超出歌曲时长或顺序倒退的解析结果，避免错误候选进入长期缓存。
pub(super) fn is_plausible_timeline(track: &TrackDescriptor, lines: &[LyricLine]) -> bool {
    matches!(
        validate_timeline(track, lines),
        TimelineValidation::Plausible
    )
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
    use crate::{
        lyrics::model::{LyricLine, LyricWord},
        media::MediaPlayer,
    };

    use super::{TimelineValidation, TrackDescriptor, is_plausible_timeline, validate_timeline};

    /// 构造具有稳定时长的测试歌曲。
    fn track() -> TrackDescriptor {
        TrackDescriptor {
            key: "track".to_owned(),
            player: MediaPlayer::QqMusic,
            title: "歌曲".to_owned(),
            artists: vec!["歌手".to_owned()],
            duration_ms: Some(180_000),
        }
    }

    /// 构造一行合法的逐字歌词。
    fn line(start_ms: u64) -> LyricLine {
        LyricLine {
            start_ms,
            end_ms: start_ms + 1_000,
            text: "歌词".to_owned(),
            translation: None,
            romanization: None,
            words: vec![LyricWord {
                start_ms,
                end_ms: start_ms + 500,
                text: "歌词".to_owned(),
            }],
        }
    }

    #[test]
    fn timeline_rejects_lines_far_beyond_track_duration() {
        assert!(!is_plausible_timeline(&track(), &[line(200_000)]));
    }

    #[test]
    fn timeline_distinguishes_temporarily_short_player_duration() {
        assert!(matches!(
            validate_timeline(&track(), &[line(200_000)]),
            TimelineValidation::DurationMismatch {
                track_duration_ms: 180_000,
                latest_start_ms: 200_000,
                latest_end_ms: 201_000,
            }
        ));
    }

    #[test]
    fn timeline_rejects_unsorted_lines() {
        assert!(!is_plausible_timeline(
            &track(),
            &[line(10_000), line(9_000)]
        ));
    }

    #[test]
    fn timeline_accepts_ordered_lines_inside_duration() {
        assert!(is_plausible_timeline(
            &track(),
            &[line(10_000), line(20_000)]
        ));
    }
}
