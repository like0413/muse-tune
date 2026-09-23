use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::lyrics::{
    chinese_conversion::convert_texts,
    model::{
        LyricsResolutionOutcome, LyricsResolutionRecord, LyricsResolutionSite,
        LyricsResolutionStep, LyricsResolutionTrack,
    },
    track::TrackDescriptor,
};

use super::{LyricsRuntimeState, LyricsService};

/// 单轮解析最多保留的诊断步骤数；并行在线阶段有多个来源，过小会静默截断关键证据。
const RESOLUTION_STEP_LIMIT: usize = 16;

/// 保留的已完成解析轮数。诊断面板只对照上一轮，保留更多轮次会挤占面板且难以逐轮比对。
const RESOLUTION_HISTORY_LIMIT: usize = 1;

impl LyricsService {
    /// 为当前解析代数创建一条新的诊断链路，并记下本轮使用的曲目信息。
    pub(super) fn begin_resolution_trace(&self, generation: u64, track: &TrackDescriptor) {
        let mut metadata = Vec::with_capacity(track.artists.len() + 1);
        metadata.push(track.title.clone());
        metadata.extend(track.artists.iter().cloned());
        let mut metadata = convert_texts(metadata, self.preferences().chinese_variant).into_iter();
        let title = metadata.next().unwrap_or_default();
        if let Ok(mut state) = self.inner.runtime_state.write() {
            archive_previous_resolution(&mut state);
            state.trace_generation = generation;
            state.resolution_started_at = Some(Instant::now());
            state.resolution_duration_ms = None;
            state.resolution_finished_at_seconds = None;
            state.resolution_track = Some(LyricsResolutionTrack {
                title,
                artists: metadata.collect(),
                duration_ms: track.duration_ms,
            });
            state.resolution_steps.clear();
        }
    }

    /// 新解析代数一经接受就清除旧歌曲链路，提前返回时不展示历史结果。
    pub(super) fn reset_resolution_trace(&self, generation: u64) {
        if let Ok(mut state) = self.inner.runtime_state.write() {
            archive_previous_resolution(&mut state);
            state.trace_generation = generation;
            state.resolution_started_at = None;
            state.resolution_duration_ms = None;
            state.resolution_finished_at_seconds = None;
            state.resolution_track = None;
            state.resolution_steps.clear();
        }
    }

    /// 记录一个不属于并发阶段的解析步骤：缓存、本地与后台升级。
    pub(super) fn record_resolution_step(
        &self,
        generation: u64,
        site: LyricsResolutionSite,
        outcome: LyricsResolutionOutcome,
        detail: Option<String>,
    ) {
        self.record_parallel_resolution_step(generation, false, site, outcome, detail);
    }

    /// 记录一次在线查询，并在诊断数据中保留其并发阶段。
    pub(super) fn record_parallel_resolution_step(
        &self,
        generation: u64,
        parallel: bool,
        site: LyricsResolutionSite,
        outcome: LyricsResolutionOutcome,
        detail: Option<String>,
    ) {
        if let Ok(mut state) = self.inner.runtime_state.write()
            && state.trace_generation == generation
            && state.resolution_steps.len() < RESOLUTION_STEP_LIMIT
        {
            state.resolution_steps.push(LyricsResolutionStep {
                site,
                outcome,
                detail,
                parallel,
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
            state.resolution_finished_at_seconds = Some(now_seconds());
        }
        (self.inner.diagnostics_notifier)();
    }
}

/// 把上一轮链路移入历史；空链路与仍在运行的轮次都不记录。
///
/// 步骤与最终结论一并保留：只留最近一次时，缓存命中那轮会把真正联网那轮挤出诊断，
/// 让“为什么不显示歌词”失去唯一可用的证据。
fn archive_previous_resolution(state: &mut LyricsRuntimeState) {
    if state.resolution_steps.is_empty() {
        return;
    }
    // 仍在运行的一轮没有结论也没有耗时；历史只留一条，归档它只会把真正完成的上一轮挤掉。
    let Some(finished_at_seconds) = state.resolution_finished_at_seconds else {
        return;
    };
    state.recent_resolutions.insert(
        0,
        LyricsResolutionRecord {
            finished_at_seconds: Some(finished_at_seconds),
            duration_ms: state.resolution_duration_ms,
            track: state.resolution_track.clone(),
            steps: std::mem::take(&mut state.resolution_steps),
            status: state.snapshot.status,
            source: state.snapshot.source.clone(),
            precision: state.snapshot.precision,
            error_reason: state.snapshot.error_reason.clone(),
        },
    );
    state.recent_resolutions.truncate(RESOLUTION_HISTORY_LIMIT);
}

/// 将内部计时安全转换为诊断使用的毫秒值。
pub(super) fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// 记录完成时刻用的墙上时钟秒数。
fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
