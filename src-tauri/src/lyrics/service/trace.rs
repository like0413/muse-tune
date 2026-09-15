use std::time::{Duration, Instant};

use super::{LyricsResolutionOutcome, LyricsResolutionStep, LyricsService};

impl LyricsService {
    /// 为当前解析代数创建一条新的诊断链路。
    pub(super) fn begin_resolution_trace(&self, generation: u64) {
        if let Ok(mut state) = self.inner.runtime_state.write() {
            state.trace_generation = generation;
            state.resolution_started_at = Some(Instant::now());
            state.resolution_duration_ms = None;
            state.resolution_steps.clear();
        }
    }

    /// 新解析代数一经接受就清除旧歌曲链路，提前返回时不展示历史结果。
    pub(super) fn reset_resolution_trace(&self, generation: u64) {
        if let Ok(mut state) = self.inner.runtime_state.write() {
            state.trace_generation = generation;
            state.resolution_started_at = None;
            state.resolution_duration_ms = None;
            state.resolution_steps.clear();
        }
    }

    /// 记录一个串行解析步骤。
    pub(super) fn record_resolution_step(
        &self,
        generation: u64,
        label: &str,
        outcome: LyricsResolutionOutcome,
        detail: Option<String>,
    ) {
        self.record_resolution_step_in_group(generation, None, label, outcome, detail);
    }

    /// 记录一次在线查询，并在诊断数据中保留其并发阶段。
    pub(super) fn record_resolution_step_in_group(
        &self,
        generation: u64,
        parallel_group: Option<&str>,
        label: &str,
        outcome: LyricsResolutionOutcome,
        detail: Option<String>,
    ) {
        if let Ok(mut state) = self.inner.runtime_state.write()
            && state.trace_generation == generation
            && state.resolution_steps.len() < 8
        {
            state.resolution_steps.push(LyricsResolutionStep {
                label: label.to_owned(),
                outcome,
                detail,
                parallel_group: parallel_group.map(str::to_owned),
            });
        }
    }

    /// 完成当前代数的诊断计时，并通知只读诊断界面刷新。
    pub(super) fn finish_resolution_trace(&self, generation: u64) {
        if let Ok(mut state) = self.inner.runtime_state.write()
            && state.trace_generation == generation
        {
            state.resolution_duration_ms = state
                .resolution_started_at
                .take()
                .map(|started_at| duration_millis(started_at.elapsed()));
        }
        (self.inner.diagnostics_notifier)();
    }
}

/// 将内部计时安全转换为诊断使用的毫秒值。
pub(super) fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
