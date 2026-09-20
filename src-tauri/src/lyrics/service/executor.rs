use std::time::Instant;

use crate::lyrics::{
    error::LyricsError,
    model::{LyricsLookupOutcome, LyricsResolutionOutcome},
    network::ResolutionDeadline,
};

use super::pipeline::player_label;
use super::{
    LyricsCandidate, LyricsParallelGroup, LyricsResolutionResult, LyricsService, TrackDescriptor,
    candidate_from_source, players, summarize_resolution_result, timeline_rejection_reason,
};
use super::{plan::ResolutionAttempt, plan::ResolutionCapability, trace::duration_millis};

/// 单次来源调用及其耗时，允许并发完成后仍按计划顺序记录诊断。
pub(super) struct AttemptExecution {
    attempt: ResolutionAttempt,
    duration_ms: u64,
    result: LyricsResolutionResult,
}

/// 统一归类后的来源执行结果。
pub(super) enum RecordedAttempt {
    Candidate(LyricsCandidate),
    /// 来源正常执行，但没有产出可用的歌词结论。
    Missed,
    /// 来源技术性失败（网络、超时、适配器错误）；与“确认没有歌词”必须区分开。
    Failed,
    Cancelled,
}

impl LyricsService {
    /// 调用一个本地或在线适配器，不在工作线程内修改诊断状态。
    pub(super) fn execute_attempt(
        &self,
        attempt: ResolutionAttempt,
        track: &TrackDescriptor,
        deadline: &ResolutionDeadline,
    ) -> AttemptExecution {
        let started_at = Instant::now();
        let cache_path = self.cache_path(attempt.player);
        let result = match attempt.capability {
            ResolutionCapability::Local => {
                players::resolve_local_for(attempt.player, track, cache_path)
            }
            ResolutionCapability::Online => players::resolve_online_for(
                attempt.player,
                track,
                cache_path,
                &self.inner.client,
                deadline,
            ),
        };
        AttemptExecution {
            attempt,
            duration_ms: duration_millis(started_at.elapsed()),
            result,
        }
    }

    /// 校验来源结果、写入统一诊断，并转换为候选或控制信号。
    pub(super) fn record_attempt(
        &self,
        execution: AttemptExecution,
        track: &TrackDescriptor,
        generation: u64,
        group: Option<LyricsParallelGroup>,
    ) -> RecordedAttempt {
        let AttemptExecution {
            attempt,
            duration_ms,
            result,
        } = execution;
        if let Ok(LyricsLookupOutcome::Hit(resolved)) = &result
            && let Some(reason) = timeline_rejection_reason(track, &resolved.lines)
        {
            self.record_resolution_step_in_group(
                generation,
                group,
                attempt.site,
                LyricsResolutionOutcome::Error,
                Some(format!(
                    "{} · {reason} · {duration_ms} ms",
                    player_label(attempt.player)
                )),
            );
            // 步骤本身是失败的，但这属于“来源给出的内容不可用”，不是网络或适配器故障，
            // 因此不改变整首歌的结论方向。
            return RecordedAttempt::Missed;
        }

        let (outcome, detail) = summarize_resolution_result(&result);
        // 命中步骤自带来源信息；未命中的步骤没有，带上平台名才能看出是哪个平台没给结果。
        let detail = if matches!(outcome, LyricsResolutionOutcome::Hit) {
            detail.map_or_else(
                || format!("{duration_ms} ms"),
                |detail| format!("{detail} · {duration_ms} ms"),
            )
        } else {
            detail.map_or_else(
                || format!("{} · {duration_ms} ms", player_label(attempt.player)),
                |detail| {
                    format!(
                        "{} · {detail} · {duration_ms} ms",
                        player_label(attempt.player)
                    )
                },
            )
        };
        self.record_resolution_step_in_group(
            generation,
            group,
            attempt.site,
            outcome,
            Some(detail),
        );
        match result {
            Ok(LyricsLookupOutcome::Hit(resolved)) => {
                RecordedAttempt::Candidate(candidate_from_source(resolved))
            }
            Ok(LyricsLookupOutcome::Unsupported | LyricsLookupOutcome::Miss(_)) => {
                RecordedAttempt::Missed
            }
            Err(LyricsError::Cancelled) => RecordedAttempt::Cancelled,
            Err(error) => {
                log::warn!("{:?} 歌词来源失败: {error}", attempt.site);
                RecordedAttempt::Failed
            }
        }
    }
}
