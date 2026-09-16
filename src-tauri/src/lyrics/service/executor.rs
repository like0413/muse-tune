use std::time::Instant;

use crate::lyrics::{
    error::LyricsError,
    model::{LyricsLookupOutcome, LyricsResolutionOutcome},
    network::ResolutionDeadline,
};

use super::{
    LyricsCandidate, LyricsResolutionResult, LyricsService, TrackDescriptor, candidate_from_source,
    is_plausible_timeline, players, summarize_resolution_result,
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
    Continue,
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
        parallel_group: Option<&str>,
    ) -> RecordedAttempt {
        let AttemptExecution {
            attempt,
            duration_ms,
            result,
        } = execution;
        if let Ok(LyricsLookupOutcome::Hit(resolved)) = &result
            && !is_plausible_timeline(track, &resolved.lines)
        {
            self.record_resolution_step_in_group(
                generation,
                parallel_group,
                attempt.label,
                LyricsResolutionOutcome::Error,
                Some(format!("歌词时间轴超出歌曲有效范围 · {duration_ms} ms")),
            );
            return RecordedAttempt::Continue;
        }

        let (outcome, detail) = summarize_resolution_result(&result);
        self.record_resolution_step_in_group(
            generation,
            parallel_group,
            attempt.label,
            outcome,
            Some(detail.map_or_else(
                || format!("{duration_ms} ms"),
                |detail| format!("{detail} · {duration_ms} ms"),
            )),
        );
        match result {
            Ok(LyricsLookupOutcome::Hit(resolved)) => {
                RecordedAttempt::Candidate(candidate_from_source(resolved))
            }
            Ok(LyricsLookupOutcome::Unsupported | LyricsLookupOutcome::Miss(_)) => {
                RecordedAttempt::Continue
            }
            Err(LyricsError::Cancelled) => RecordedAttempt::Cancelled,
            Err(error) => {
                log::warn!("{}失败: {error}", attempt.label);
                RecordedAttempt::Continue
            }
        }
    }
}
