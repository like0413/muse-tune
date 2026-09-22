//! 一轮歌词解析的执行：按计划尝试各来源，选出最佳候选并提交结论。
//!
//! 这里的时序约束集中在两处，改动时容易踩到：
//! - 每次发布前都要重新校验解析代数，切歌会让在途结果作废；
//! - “没有歌词”这类结论下结论前要留等待窗口，因为切歌瞬间 GSMTC 的标题与时间线
//!   可能不同步，用混搭快照去匹配会让所有来源同时未命中。

use std::{
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::{Duration, Instant},
};

use crate::lyrics::{
    cache::CacheLookup,
    error::LyricsError,
    matcher::MAX_DURATION_DIFFERENCE_MS,
    model::{
        LyricsLookupOutcome, LyricsPrecision, LyricsResolutionMethod, LyricsResolutionOutcome,
        LyricsResolutionSite, LyricsSnapshot, LyricsStatus, ResolvedLyrics, has_word_timing,
        platform_notice,
    },
    network::ResolutionDeadline,
    players,
    track::TrackDescriptor,
};

use super::{
    LyricsService, cache_policy,
    executor::RecordedAttempt,
    pipeline::{
        LyricsCandidate, TimelineValidation, format_milliseconds, is_acceptable_candidate,
        is_cached_snapshot_displayable, lookup_miss_detail, select_best_candidate,
        timeline_rejection_reason, validate_timeline,
    },
    plan::ResolutionPlan,
    trace::duration_millis,
};

/// 全部来源未命中时，结论先挂起这么久的窗口，用来等媒体把切歌瞬间的时间线补齐。
const CONCLUSION_SETTLE_TIMEOUT: Duration = Duration::from_millis(500);
/// 挂起期间检查“是否已有新一轮解析”的间隔；也是它对新请求的最大延迟。
const CONCLUSION_SETTLE_POLL: Duration = Duration::from_millis(20);

/// 一轮解析在阶段之间累积的结果。
///
/// 缓存升级、本地、在线三个阶段各自只往这里追加，最终由 `finish_resolution` 统一择优；
/// 收敛成结构体后阶段之间不再需要传递一长串出入参。
struct ResolutionProgress {
    /// 各阶段收集到的候选。
    candidates: Vec<LyricsCandidate>,
    /// 已经展示了可用的逐行结果（缓存命中或本地命中）时，本轮在线只用于升级到逐字：
    /// 换成另一个逐行只会让用户看到歌词无意义地跳一次，精度并没有提高。
    upgrade_to_word_only: bool,
    /// 是否有来源技术性失败；用于把“来源暂时不可用”与“确认这首歌没有歌词”分开上报。
    source_failed: bool,
    /// 缓存升级阶段是否已经查过当前播放器的本地来源，此时本地阶段不必重复。
    local_source_resolved: bool,
    /// 缓存里已有可展示的逐行结果；本地阶段以它为基准判断替换是否值得。
    cached_line_displayed: bool,
}

impl LyricsService {
    pub(super) fn resolve_track(
        &self,
        track: TrackDescriptor,
        generation: u64,
        cancellation: Arc<AtomicBool>,
        cached: Option<CacheLookup>,
        cache_cleared: bool,
    ) {
        let deadline = ResolutionDeadline::new(cancellation);
        let preferences = self.preferences();
        let plan = ResolutionPlan::new(
            track.player,
            preferences.online_strategy,
            &preferences.online_sources,
        );
        let cache_timeline_validation = cached.as_ref().and_then(|cached| {
            (cached.snapshot.status == LyricsStatus::Ready)
                .then(|| validate_timeline(&track, &cached.snapshot.lines))
        });
        let cached_snapshot_displayable = cached
            .as_ref()
            .is_some_and(|cached| is_cached_snapshot_displayable(&track, &cached.snapshot));
        self.record_resolution_step(
            generation,
            LyricsResolutionSite::ApplicationCache,
            if cached.is_some() {
                LyricsResolutionOutcome::Hit
            } else {
                LyricsResolutionOutcome::Miss
            },
            Some(cached.as_ref().map_or_else(
                || cache_miss_detail(cache_cleared),
                |cached| describe_cache_step(cached, cache_timeline_validation),
            )),
        );
        // 过期的语义结论（纯音乐 / 没有歌词）不携带歌词行，无法参与候选排序，但保留为最终兜底：
        // 否则“曾经确认过纯音乐”会在所有来源未命中时退化成“没有找到可靠歌词”。
        let stale_notice = cached
            .as_ref()
            .filter(|cached| {
                !cached.is_fresh
                    && matches!(
                        cached.snapshot.status,
                        LyricsStatus::Instrumental | LyricsStatus::NoLyrics
                    )
            })
            .map(|cached| cached.snapshot.clone());

        // 缓存里已有可展示的逐行结果：先展示它，本轮在线只用于升级到逐字，不再降级替换。
        let cached_line_displayed = cached.as_ref().is_some_and(|cached| {
            cached.is_fresh
                && cached.snapshot.status == LyricsStatus::Ready
                && cached.snapshot.precision == Some(LyricsPrecision::Line)
        });
        let mut progress = ResolutionProgress {
            candidates: Vec::new(),
            upgrade_to_word_only: cached_line_displayed,
            source_failed: false,
            local_source_resolved: false,
            cached_line_displayed,
        };

        // 缓存分支可能已经解析过当前播放器的本地来源（新鲜逐行的升级判定），此时本地阶段不必重复。
        if let Some(cached) = cached {
            if cached_snapshot_displayable && cached.is_fresh {
                // 逐行与纯音乐都要顺手查一次本地（判据见 `should_check_local_upgrade`）。
                let should_check_local = cache_policy::should_check_local_upgrade(&cached.snapshot);
                // 在线复核只为把逐行升级成逐字；纯音乐不联网，联网也推不翻它。
                let should_revalidate = cached.needs_revalidation && preferences.allow_online;
                self.publish_if_current_with_method(
                    cached.snapshot.clone(),
                    generation,
                    LyricsResolutionMethod::ApplicationCache,
                );
                if !should_check_local || !self.is_current_generation(generation) {
                    return;
                }

                // 已有缓存必须先展示；本地精度升级属于增强路径，不能阻塞首屏歌词。
                progress.local_source_resolved = true;
                let Some(upgraded_locally) = self.try_local_upgrade(&track, generation, &cached)
                else {
                    return;
                };
                // 本地升级没有结果时，只有逐行缓存继续走在线阶段尝试升级到逐字。
                if !should_revalidate || upgraded_locally {
                    return;
                }
            }
            // 可参与比较的缓存：过期但仍可展示的快照，或需要升级复核的新鲜逐行快照
            // （逐行只接受逐字替换，见 `improves_displayed`）。
            if cached.snapshot.status == LyricsStatus::Ready
                && cached_snapshot_displayable
                && let Some(source) = cached.snapshot.source
            {
                progress.candidates.push(LyricsCandidate {
                    resolved: ResolvedLyrics {
                        source,
                        lines: cached.snapshot.lines,
                    },
                    resolution_method: LyricsResolutionMethod::ApplicationCache,
                });
            }
        }

        if !self.run_local_stage(&track, generation, &plan, &deadline, &mut progress) {
            return;
        }
        if !preferences.allow_online {
            self.finish_resolution(
                &track,
                generation,
                progress,
                stale_notice,
                "联网策略仅允许本地与缓存",
                "本地与缓存歌词来源暂时不可用",
            );
            return;
        }
        if !self.run_online_stages(&track, generation, &plan, &deadline, &mut progress) {
            return;
        }
        self.finish_resolution(
            &track,
            generation,
            progress,
            stale_notice,
            "没有找到可靠歌词",
            "歌词来源暂时不可用，请检查网络连接",
        );
    }

    /// 已有新鲜缓存时，检查本地歌词是否提供更高精度或更完整的辅助内容。
    ///
    /// 返回 `None` 表示本轮解析已被取消，调用方应当直接结束。
    fn try_local_upgrade(
        &self,
        track: &TrackDescriptor,
        generation: u64,
        cached: &CacheLookup,
    ) -> Option<bool> {
        let mut upgraded_locally = false;
        let upgrade_started_at = Instant::now();
        match players::resolve_current_local(track, self.cache_path(track.player)) {
            Ok(LyricsLookupOutcome::Hit(local))
                if is_acceptable_candidate(track, &local.lines)
                    && cache_policy::local_result_is_upgrade(&cached.snapshot, &local) =>
            {
                self.record_resolution_step(
                    generation,
                    LyricsResolutionSite::LocalUpgrade,
                    LyricsResolutionOutcome::Hit,
                    Some(format!(
                        "发现更高精度或辅助内容更完整的本地歌词 · {} ms",
                        duration_millis(upgrade_started_at.elapsed()),
                    )),
                );
                self.publish_resolution(track, local, generation);
                upgraded_locally = true;
            }
            Ok(LyricsLookupOutcome::Hit(local))
                if !is_acceptable_candidate(track, &local.lines) =>
            {
                self.record_resolution_step(
                    generation,
                    LyricsResolutionSite::LocalUpgrade,
                    LyricsResolutionOutcome::Error,
                    Some(format!(
                        "{} · {} ms",
                        timeline_rejection_reason(track, &local.lines)
                            .unwrap_or("歌词时间轴不可用"),
                        duration_millis(upgrade_started_at.elapsed()),
                    )),
                );
            }
            Ok(LyricsLookupOutcome::Hit(_)) => self.record_resolution_step(
                generation,
                LyricsResolutionSite::LocalUpgrade,
                LyricsResolutionOutcome::Miss,
                Some(format!(
                    "本地歌词未提供更高精度或更多辅助内容 · {} ms",
                    duration_millis(upgrade_started_at.elapsed()),
                )),
            ),
            Ok(LyricsLookupOutcome::Miss(reason)) => self.record_resolution_step(
                generation,
                LyricsResolutionSite::LocalUpgrade,
                LyricsResolutionOutcome::Miss,
                Some(format!(
                    "{} · {} ms",
                    lookup_miss_detail(reason),
                    duration_millis(upgrade_started_at.elapsed()),
                )),
            ),
            Ok(LyricsLookupOutcome::Unsupported) => self.record_resolution_step(
                generation,
                LyricsResolutionSite::LocalUpgrade,
                LyricsResolutionOutcome::Miss,
                Some(format!(
                    "当前播放器不支持本地歌词 · {} ms",
                    duration_millis(upgrade_started_at.elapsed()),
                )),
            ),
            Err(LyricsError::Cancelled) => return None,
            Err(error) => {
                self.record_resolution_step(
                    generation,
                    LyricsResolutionSite::LocalUpgrade,
                    LyricsResolutionOutcome::Error,
                    Some(format!(
                        "{error} · {} ms",
                        duration_millis(upgrade_started_at.elapsed()),
                    )),
                );
                log::debug!("检查播放器本地歌词升级失败: {error}");
            }
        }
        Some(upgraded_locally)
    }

    /// 执行本地阶段：当前播放器的本地歌词是同一首歌最可信的来源。
    ///
    /// 返回 `false` 表示本轮解析到此结束（已发布逐字或语义结论、被取消或代际作废）。
    fn run_local_stage(
        &self,
        track: &TrackDescriptor,
        generation: u64,
        plan: &ResolutionPlan,
        deadline: &ResolutionDeadline,
        progress: &mut ResolutionProgress,
    ) -> bool {
        let local_attempts = plan
            .local_attempts
            .iter()
            .filter(|attempt| !(progress.local_source_resolved && attempt.player == track.player));
        for attempt in local_attempts {
            let execution = self.execute_attempt(*attempt, track, deadline);
            match self.record_attempt(execution, track, generation, false) {
                RecordedAttempt::Candidate(candidate)
                    if candidate.resolved.source.player == track.player
                        && has_word_timing(&candidate.resolved.lines) =>
                {
                    self.publish_candidate(track, candidate, generation);
                    return false;
                }
                // 占位文案（“此歌曲为没有填词的纯音乐，请您欣赏”）是播放器对这首歌给出的结论，
                // 与缓存里的语义结论等价：同一平台的在线接口只会返回同一句话，其他平台的歌词
                // 也推不翻已展示的结论（结论型缓存本来就不联网复核）。本地已给出结论就地收口，
                // 不再发起在线查询。
                RecordedAttempt::Candidate(candidate)
                    if candidate.resolved.source.player == track.player
                        && platform_notice(&candidate.resolved.lines).is_some() =>
                {
                    self.publish_candidate(track, candidate, generation);
                    return false;
                }
                RecordedAttempt::Candidate(candidate) => {
                    // 本地逐行先发布：用户不必等在线阶段跑完才看到歌词，升级在后台继续。
                    // 但已展示逐行时（新鲜缓存）只有真正的提升才值得替换，否则歌词会无意义地跳一次。
                    if candidate.resolved.source.player == track.player
                        && improves_displayed(progress.cached_line_displayed, &candidate)
                    {
                        progress.upgrade_to_word_only = true;
                        self.publish_candidate(track, candidate.clone(), generation);
                    }
                    progress.candidates.push(candidate);
                }
                RecordedAttempt::Missed => {}
                RecordedAttempt::Failed => progress.source_failed = true,
                RecordedAttempt::Cancelled => return false,
            }
            if !self.is_current_generation(generation) {
                return false;
            }
        }
        true
    }

    /// 执行全部在线阶段：同一阶段内的来源并发查询，候选按计划顺序收集。
    ///
    /// 返回 `false` 表示本轮解析已被取消或代际作废，调用方应当直接结束。
    fn run_online_stages(
        &self,
        track: &TrackDescriptor,
        generation: u64,
        plan: &ResolutionPlan,
        deadline: &ResolutionDeadline,
        progress: &mut ResolutionProgress,
    ) -> bool {
        for stage in &plan.online_stages {
            if stage.attempts.is_empty() {
                continue;
            }
            let executions = if stage.attempts.len() == 1 {
                vec![self.execute_attempt(stage.attempts[0], track, deadline)]
            } else {
                thread::scope(|scope| {
                    let service = self;
                    let handles = stage
                        .attempts
                        .iter()
                        .copied()
                        .map(|attempt| {
                            scope.spawn(move || service.execute_attempt(attempt, track, deadline))
                        })
                        .collect::<Vec<_>>();
                    handles
                        .into_iter()
                        .filter_map(|handle| handle.join().ok())
                        .collect::<Vec<_>>()
                })
            };
            for execution in executions {
                match self.record_attempt(execution, track, generation, stage.parallel) {
                    RecordedAttempt::Candidate(candidate) => progress.candidates.push(candidate),
                    RecordedAttempt::Missed => {}
                    RecordedAttempt::Failed => progress.source_failed = true,
                    RecordedAttempt::Cancelled => return false,
                }
            }
            if !self.is_current_generation(generation) {
                return false;
            }
        }
        true
    }

    /// 选出并提交最终结论。
    ///
    /// 离线与在线两条路径的收尾完全一致，只有“确认没有歌词”时的原因文案不同，
    /// 因此合并为一处：否则两边容易各自漂移，出现同一条件下结论不一致。
    fn finish_resolution(
        &self,
        track: &TrackDescriptor,
        generation: u64,
        progress: ResolutionProgress,
        stale_notice: Option<LyricsSnapshot>,
        miss_reason: &str,
        failure_reason: &str,
    ) {
        // 择优顺序取自当前的在线接口配置：本轮候选已按计划抓取完毕，这里读最新快照即可——
        // 偏好变化不会作废进行中的一轮，而是从下一轮解析开始生效。
        let online_sources = self.preferences().online_sources;
        match select_best_candidate(track, progress.candidates, &online_sources) {
            Some(candidate) if improves_displayed(progress.upgrade_to_word_only, &candidate) => {
                self.publish_candidate(track, candidate, generation);
            }
            // 只升级不降级：已经展示的逐行保持不变，也不要让过期结论覆盖它。
            Some(_) => {}
            None => {
                // 混搭快照（新标题 + 上一首时间线）会让所有来源同时未命中，此时下结论是错的：
                // 先确认时长仍属于本曲，再留一个等待窗口，等修正后的描述符触发新一轮解析。
                if !self.round_duration_is_current(track)
                    || self.superseded_before_conclusion(generation)
                {
                    log::debug!("本轮解析的输入可能已过期，放弃本次结论");
                } else if let Some(snapshot) = stale_notice {
                    // 沿用上次的语义结论；顺带写回缓存以刷新时间戳，避免它成为永不更新的僵死条目。
                    self.store_and_publish_if_current(
                        snapshot,
                        generation,
                        LyricsResolutionMethod::ApplicationCache,
                    );
                } else {
                    self.publish_no_lyrics(
                        &track.key,
                        generation,
                        progress.source_failed,
                        miss_reason,
                        failure_reason,
                    );
                }
            }
        }
    }

    /// 本轮使用的时长是否仍然属于当前曲目。
    ///
    /// 切歌瞬间 SMTC 的媒体属性与时间线是两条独立通道：标题可能已经换成新歌，时间线还留在上一首。
    /// 用这种时长去匹配，所有来源都会被时长这一关否掉，于是一首有词的歌被判成“没有歌词”。
    /// 判据与 `update_track` 保持一致：只有超过匹配容差的变化才算换歌，小幅修正不该让结论失效。
    fn round_duration_is_current(&self, track: &TrackDescriptor) -> bool {
        let Ok(current) = self.inner.current_track.lock() else {
            return true;
        };
        let Some(current) = current.as_ref().filter(|current| current.key == track.key) else {
            return true;
        };
        match (current.duration_ms, track.duration_ms) {
            (Some(current), Some(round)) => current.abs_diff(round) <= MAX_DURATION_DIFFERENCE_MS,
            (None, None) => true,
            _ => false,
        }
    }

    /// 在“没有歌词 / 获取失败”这类结论发布前留出的等待窗口。
    ///
    /// 混搭快照只有等到时间线补齐才会暴露：修正后的描述符会触发新一轮解析（代数变化），
    /// 所以这里只需确认本轮仍是当前代数。返回 `true` 表示已有新一轮接手，本次结论作废。
    fn superseded_before_conclusion(&self, generation: u64) -> bool {
        let started_at = Instant::now();
        while started_at.elapsed() < CONCLUSION_SETTLE_TIMEOUT {
            if !self.is_current_generation(generation) {
                return true;
            }
            thread::sleep(CONCLUSION_SETTLE_POLL);
        }
        false
    }
}

/// 候选是否可以替换当前已展示的结果。
///
/// 已经展示逐行时只接受逐字：在线阶段拿到的另一个逐行精度并没有提高，替换只会让用户
/// 看到歌词无意义地跳一次。
fn improves_displayed(upgrade_to_word_only: bool, candidate: &LyricsCandidate) -> bool {
    !upgrade_to_word_only || has_word_timing(&candidate.resolved.lines)
}

/// 起轮时没有缓存的原因文案，区分“本来就没有”与“本次流程刚清除”。
///
/// 手动刷新、切换联网策略和本地歌词升级都会先删掉缓存再重新解析，笼统写成“无应用缓存”
/// 会让人以为这首歌从未有过缓存。
fn cache_miss_detail(cleared: bool) -> String {
    if cleared {
        "已由本次流程清除，重新完整解析".to_owned()
    } else {
        "解析开始时无应用缓存".to_owned()
    }
}

/// 缓存命中步骤的诊断文案；区分语义结论、逐字、逐行与时间轴异常。
fn describe_cache_step(cached: &CacheLookup, validation: Option<TimelineValidation>) -> String {
    if let Some(label) = notice_status_label(cached.snapshot.status) {
        return if cached.is_fresh {
            format!("已确认{label}，有效期内")
        } else {
            format!("已确认{label}，已过期，重新确认")
        };
    }
    match validation {
        Some(TimelineValidation::Plausible) if cached.is_fresh => {
            if cached.needs_revalidation {
                "有效期内，继续确认能否升级到逐字".to_owned()
            } else {
                "有效期内".to_owned()
            }
        }
        Some(TimelineValidation::Plausible) => "已过期，作为兜底候选".to_owned(),
        Some(TimelineValidation::DurationMismatch {
            track_duration_ms,
            latest_start_ms,
            latest_end_ms,
        }) => format!(
            "已读取；播放器时长 {}，歌词末行开始 {}、结束 {}；采用有效缓存",
            format_milliseconds(track_duration_ms),
            format_milliseconds(latest_start_ms),
            format_milliseconds(latest_end_ms),
        ),
        Some(TimelineValidation::Invalid(_)) | None => {
            "已读取，但缓存状态或时间轴结构无效".to_owned()
        }
    }
}

/// 语义结论的中文名；真歌词与其他状态返回 `None`。
fn notice_status_label(status: LyricsStatus) -> Option<&'static str> {
    match status {
        LyricsStatus::Instrumental => Some("纯音乐"),
        LyricsStatus::NoLyrics => Some("没有歌词"),
        LyricsStatus::Loading
        | LyricsStatus::Ready
        | LyricsStatus::Unavailable
        | LyricsStatus::Error => None,
    }
}
