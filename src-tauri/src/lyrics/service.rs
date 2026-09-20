use std::{
    collections::{HashMap, HashSet},
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use reqwest::{Url, blocking::Client, redirect};
use tauri::{Emitter, Manager, Runtime};

use crate::media::{MediaPlayer, MediaSessionSnapshot};
use crate::native_defaults;

use super::{
    cache::{CacheLookup, ParsedLyricsCache},
    error::LyricsError,
    matcher::MAX_DURATION_DIFFERENCE_MS,
    model::{
        LyricsAdapterDiagnostics, LyricsCacheDiagnostics, LyricsDiagnostics, LyricsLookupOutcome,
        LyricsOnlineStrategy, LyricsParallelGroup, LyricsPrecision, LyricsResolutionMethod,
        LyricsResolutionOutcome, LyricsResolutionRecord, LyricsResolutionSite,
        LyricsResolutionStep, LyricsResolutionTrack, LyricsSnapshot, LyricsSnapshotDiagnostics,
        LyricsStatus, ResolvedLyrics, has_word_timing,
    },
    network::ResolutionDeadline,
    players,
    track::TrackDescriptor,
    watcher::{LyricsFileWatcher, LyricsWatcherMetrics},
};
use players::RegistryWatchHandle;

mod cache_policy;
mod executor;
mod pipeline;
mod plan;
mod trace;
mod watch_coordinator;

use executor::RecordedAttempt;
use pipeline::{
    LyricsCandidate, TimelineValidation, candidate_from_source, format_milliseconds,
    is_acceptable_candidate, is_cached_snapshot_displayable, lookup_hit, lookup_miss_detail,
    select_best_candidate, summarize_resolution_result, timeline_rejection_reason,
    validate_timeline,
};
use plan::ResolutionPlan;
use trace::duration_millis;

const LYRICS_CHANGED_EVENT: &str = "lyrics://changed";
const LYRICS_DIAGNOSTICS_CHANGED_EVENT: &str = "lyrics://diagnostics-changed";
const NETWORK_TIMEOUT: Duration = Duration::from_secs(8);
/// 全部来源未命中时，结论先挂起这么久的窗口，用来等媒体把切歌瞬间的时间线补齐。
const CONCLUSION_SETTLE_TIMEOUT: Duration = Duration::from_millis(500);
/// 挂起期间检查"是否已有新一轮解析"的间隔；也是它对新请求的最大延迟。
const CONCLUSION_SETTLE_POLL: Duration = Duration::from_millis(20);
const ALLOWED_HTTPS_HOSTS: [&str; 6] = [
    "c.y.qq.com",
    "u.y.qq.com",
    "music.163.com",
    "beta-luna.douyin.com",
    "krcs.kugou.com",
    "lyrics2.kugou.com",
];

type SnapshotPublisher = dyn Fn(&LyricsSnapshot) + Send + Sync;
type DiagnosticsNotifier = dyn Fn() + Send + Sync;
type LyricsResolutionResult = Result<LyricsLookupOutcome, LyricsError>;

/// 可被 Tauri command 和媒体监控线程安全共享的歌词服务。
#[derive(Clone)]
pub struct LyricsService {
    inner: Arc<LyricsServiceInner>,
}

struct LyricsServiceInner {
    cache: ParsedLyricsCache,
    client: Client,
    current_track: Mutex<Option<TrackDescriptor>>,
    generation: AtomicU64,
    preferences: RwLock<LyricsPreferences>,
    publisher: Arc<SnapshotPublisher>,
    diagnostics_notifier: Arc<DiagnosticsNotifier>,
    runtime_state: RwLock<LyricsRuntimeState>,
    adapter_paths: RwLock<HashMap<MediaPlayer, Option<PathBuf>>>,
    watchers: Mutex<HashMap<MediaPlayer, LyricsFileWatcher>>,
    watcher_metrics: Arc<LyricsWatcherMetrics>,
    registry_watchers: Mutex<Vec<RegistryWatchHandle>>,
    shutdown_requested: AtomicBool,
    resolver: Mutex<ResolverState>,
}

/// 需要作为一个快照提交和读取的歌词运行偏好。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LyricsPreferences {
    enabled: bool,
    allow_online: bool,
    online_strategy: LyricsOnlineStrategy,
}

impl Default for LyricsPreferences {
    /// 状态不可用时回退到与前端共用同一份数据的默认偏好。
    fn default() -> Self {
        let defaults = &native_defaults::shared().taskbar.lyrics;
        Self {
            enabled: defaults.enabled,
            allow_online: defaults.network_policy.allows_online(),
            online_strategy: defaults.online_strategy,
        }
    }
}

#[derive(Default)]
struct ResolverState {
    running: bool,
    pending: Option<ResolutionRequest>,
    active_cancellation: Option<Arc<AtomicBool>>,
}

struct ResolutionRequest {
    track: TrackDescriptor,
    generation: u64,
    cancellation: Arc<AtomicBool>,
    cached: Option<CacheLookup>,
    /// 起轮时没有缓存是否因为本次流程主动清除了它；只影响诊断文案。
    cache_cleared: bool,
}

#[derive(Default)]
struct LyricsRuntimeState {
    snapshot: LyricsSnapshot,
    resolution_method: LyricsResolutionMethod,
    trace_generation: u64,
    resolution_started_at: Option<Instant>,
    resolution_duration_ms: Option<u64>,
    resolution_steps: Vec<LyricsResolutionStep>,
    resolution_track: Option<LyricsResolutionTrack>,
    resolution_finished_at_seconds: Option<u64>,
    recent_resolutions: Vec<LyricsResolutionRecord>,
}

impl LyricsService {
    /// 初始化缓存、匿名 HTTPS 客户端和设置快照。
    pub fn initialize<R: Runtime>(app: &tauri::App<R>) -> Result<Self, io::Error> {
        let cache_dir = app.path().app_cache_dir().map_err(io::Error::other)?;
        let cache = ParsedLyricsCache::new(&cache_dir)?;
        let client = Client::builder()
            .timeout(NETWORK_TIMEOUT)
            .connect_timeout(Duration::from_secs(4))
            .redirect(redirect::Policy::custom(|attempt| {
                if is_allowed_url(attempt.url()) && attempt.previous().len() < 3 {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .map_err(io::Error::other)?;
        let app_handle = app.handle().clone();
        let diagnostics_handle = app.handle().clone();
        let publisher = Arc::new(move |snapshot: &LyricsSnapshot| {
            if let Err(error) = app_handle.emit(LYRICS_CHANGED_EVENT, snapshot) {
                log::warn!("广播歌词状态失败: {error}");
            }
        });
        let diagnostics_notifier = Arc::new(move || {
            if let Err(error) = diagnostics_handle.emit(LYRICS_DIAGNOSTICS_CHANGED_EVENT, ()) {
                log::warn!("广播歌词诊断变化失败: {error}");
            }
        });
        let (enabled, allow_online, online_strategy) =
            super::settings::restore_lyrics_preferences(app);
        let service = Self {
            inner: Arc::new(LyricsServiceInner {
                cache,
                client,
                current_track: Mutex::new(None),
                generation: AtomicU64::new(0),
                preferences: RwLock::new(LyricsPreferences {
                    enabled,
                    allow_online,
                    online_strategy,
                }),
                publisher,
                diagnostics_notifier,
                runtime_state: RwLock::new(LyricsRuntimeState::default()),
                adapter_paths: RwLock::new(HashMap::new()),
                watchers: Mutex::new(HashMap::new()),
                watcher_metrics: Arc::new(LyricsWatcherMetrics::default()),
                registry_watchers: Mutex::new(Vec::new()),
                shutdown_requested: AtomicBool::new(false),
                resolver: Mutex::new(ResolverState::default()),
            }),
        };
        service.refresh_watchers();
        service.start_registry_watcher();
        Ok(service)
    }

    /// 停止接收新的事件，取消活动解析，并同步回收全部监听线程。
    pub(crate) fn shutdown(&self) {
        if self.inner.shutdown_requested.swap(true, Ordering::AcqRel) {
            return;
        }
        self.cancel_resolution();
        // 不在持有 service mutex 时 join，避免与正在结束的 callback 形成锁等待。
        self.release_watchers();
    }

    /// 返回最近一次歌词快照。
    pub fn snapshot(&self) -> LyricsSnapshot {
        self.inner.runtime_state.read().map_or_else(
            |_| LyricsSnapshot::default(),
            |state| state.snapshot.clone(),
        )
    }

    /// 返回当前来源、自动目录与调度器状态，供设置页只读诊断。
    pub fn diagnostics(&self) -> LyricsDiagnostics {
        let current_track = self
            .inner
            .current_track
            .lock()
            .ok()
            .and_then(|track| track.clone());
        let current_player = current_track.as_ref().map(|track| track.player);
        let (resolver_running, pending_resolution) = self
            .inner
            .resolver
            .lock()
            .map_or((false, false), |resolver| {
                (resolver.running, resolver.pending.is_some())
            });
        let (
            snapshot,
            resolution_method,
            resolution_duration_ms,
            resolution_track,
            resolution_steps,
            recent_resolutions,
        ) = self.inner.runtime_state.read().map_or_else(
            |_| {
                (
                    LyricsSnapshotDiagnostics::default(),
                    LyricsResolutionMethod::None,
                    None,
                    None,
                    Vec::new(),
                    Vec::new(),
                )
            },
            |state| {
                (
                    LyricsSnapshotDiagnostics {
                        status: state.snapshot.status,
                        source: state.snapshot.source.clone(),
                        precision: state.snapshot.precision,
                        line_count: state.snapshot.lines.len(),
                        error_reason: state.snapshot.error_reason.clone(),
                    },
                    state.resolution_method,
                    state.resolution_duration_ms,
                    state.resolution_track.clone(),
                    state.resolution_steps.clone(),
                    state.recent_resolutions.clone(),
                )
            },
        );
        let active_watchers = self
            .inner
            .watchers
            .lock()
            .map(|watchers| watchers.keys().copied().collect::<HashSet<_>>())
            .unwrap_or_default();
        let discovered_paths = self.inner.adapter_paths.read().map_or_else(
            |_| Vec::new(),
            |paths| {
                players::supported_players()
                    .map(|player| {
                        let cache_path = paths.get(&player).cloned().flatten();
                        let available = cache_path.as_ref().is_some_and(|path| path.is_dir());
                        (player, cache_path, available)
                    })
                    .collect::<Vec<_>>()
            },
        );
        let (local_cache_path, local_cache_available) = current_player
            .and_then(|current_player| {
                discovered_paths
                    .iter()
                    .find(|(player, _, _)| *player == current_player)
                    .map(|(_, path, available)| (path.clone(), *available))
            })
            .unwrap_or_default();
        let adapters = discovered_paths
            .into_iter()
            .map(
                |(player, cache_path, cache_path_available)| LyricsAdapterDiagnostics {
                    player,
                    cache_path_available,
                    cache_path: cache_path.map(|path| display_path(&path)),
                    watcher_active: active_watchers.contains(&player),
                },
            )
            .collect();
        let preferences = self.preferences();
        LyricsDiagnostics {
            snapshot,
            current_player,
            enabled: preferences.enabled,
            online_strategy: preferences.online_strategy,
            resolution_method,
            local_cache_available,
            local_cache_path: local_cache_path.map(|path| display_path(&path)),
            resolver_running,
            pending_resolution,
            resolution_duration_ms,
            resolution_track,
            resolution_steps,
            recent_resolutions,
            cache: self
                .inner
                .cache
                .diagnostics(current_track.as_ref().map(|track| track.key.as_str())),
            adapters,
            watcher: {
                let metrics = self.inner.watcher_metrics.snapshot();
                super::model::LyricsWatcherDiagnostics {
                    enqueued_batches: metrics.enqueued_batches,
                    processed_batches: metrics.processed_batches,
                    coalesced_batches: metrics.coalesced_batches,
                    callback_count: metrics.callback_count,
                    pending_batches: metrics.pending_batches,
                    pending_batches_peak: metrics.pending_batches_peak,
                }
            },
        }
    }

    /// 返回缓存容量与占用摘要，供数据页独立读取。
    pub fn cache_diagnostics(&self) -> LyricsCacheDiagnostics {
        let current_track = self
            .inner
            .current_track
            .lock()
            .ok()
            .and_then(|track| track.clone());
        self.inner
            .cache
            .diagnostics(current_track.as_ref().map(|track| track.key.as_str()))
    }

    /// 返回歌词缓存根目录，例如 `lyrics`。
    pub fn cache_directory(&self) -> &std::path::Path {
        self.inner.cache.directory()
    }

    /// 清空应用管理的歌词缓存，并通知诊断页刷新缓存状态。
    ///
    /// 与 `clear_current_cache` 一样在歌曲锁内提升解析代数并取消在途任务：否则正在运行的
    /// 解析会在清理之后把结果写回，用户看到的是“刚清空又立刻出现条目”。
    pub fn clear_cache(&self) -> Result<(), io::Error> {
        if let Ok(_guard) = self.inner.current_track.lock() {
            self.inner.generation.fetch_add(1, Ordering::AcqRel);
        }
        self.cancel_resolution();
        self.inner.cache.clear()?;
        (self.inner.diagnostics_notifier)();
        Ok(())
    }

    /// 只删除当前歌曲的应用歌词缓存，不改变正在展示的歌词快照。
    pub fn clear_current_cache(&self) -> Result<(), String> {
        let (track, _) = self.prepare_current_cache_mutation()?;
        self.cancel_resolution();
        self.inner
            .cache
            .remove(&track.key)
            .map_err(|error| format!("清理当前歌曲缓存失败: {error}"))?;
        (self.inner.diagnostics_notifier)();
        Ok(())
    }

    /// 删除当前歌曲缓存并立即启动一次完整解析。
    pub fn refresh_current(&self) -> Result<(), String> {
        let (track, generation) = self.prepare_current_cache_mutation()?;
        self.cancel_resolution();
        self.inner
            .cache
            .remove(&track.key)
            .map_err(|error| format!("清理当前歌曲缓存失败: {error}"))?;
        (self.inner.diagnostics_notifier)();
        self.start_resolution(Some(track), generation, false, true);
        Ok(())
    }

    /// 在当前歌曲锁内提升解析代数，确保旧任务不能在删除之后回写缓存。
    fn prepare_current_cache_mutation(&self) -> Result<(TrackDescriptor, u64), String> {
        let current = self
            .inner
            .current_track
            .lock()
            .map_err(|_| "当前歌曲状态不可用".to_owned())?;
        let track = current
            .clone()
            .ok_or_else(|| "当前没有可清理的歌曲".to_owned())?;
        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        Ok((track, generation))
    }

    /// 原子更新歌词开关、联网能力和在线调度策略，并只触发一次必要的重新解析。
    pub fn set_preferences(
        &self,
        enabled: bool,
        allow_online: bool,
        online_strategy: LyricsOnlineStrategy,
    ) -> Result<(), String> {
        let next = LyricsPreferences {
            enabled,
            allow_online,
            online_strategy,
        };
        let previous = {
            let mut current = self
                .inner
                .preferences
                .write()
                .map_err(|_| "歌词偏好状态不可用".to_owned())?;
            if *current == next {
                return Ok(());
            }
            let previous = *current;
            *current = next;
            previous
        };
        let enabled_changed = previous.enabled != enabled;
        let online_changed = previous.allow_online != allow_online;
        let strategy_changed = previous.online_strategy != online_strategy;
        if !enabled {
            let (track_key, generation) = {
                let current = self
                    .inner
                    .current_track
                    .lock()
                    .map_err(|_| "当前歌曲状态不可用".to_owned())?;
                let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
                (current.as_ref().map(|track| track.key.clone()), generation)
            };
            self.cancel_resolution();
            // 关闭歌词后释放文件与注册表监听，避免后台线程继续为缓存写入做本地解析。
            self.release_watchers();
            self.publish_if_current(
                LyricsSnapshot::unavailable(track_key, "歌词显示已关闭"),
                generation,
            );
            return Ok(());
        }
        if enabled_changed {
            // 重新开启歌词时恢复关闭期间释放掉的文件与注册表监听。
            self.refresh_watchers();
            self.start_registry_watcher();
        }
        let cache_cleared = if strategy_changed {
            match self.clear_current_cache() {
                Ok(()) => true,
                Err(error) => {
                    log::debug!("切换在线调度策略时清理当前歌词缓存失败: {error}");
                    false
                }
            }
        } else {
            false
        };
        if enabled_changed || online_changed || strategy_changed {
            self.force_resolve_current(online_changed && allow_online, cache_cleared)
        } else {
            (self.inner.diagnostics_notifier)();
            Ok(())
        }
    }

    /// 接收媒体模块的完整快照变化，时间线轻量事件不会触发此入口。
    pub(crate) fn update_media(&self, snapshot: Option<&MediaSessionSnapshot>) {
        self.update_track(snapshot.and_then(TrackDescriptor::from_snapshot), false);
    }

    fn update_track(&self, track: Option<TrackDescriptor>, preserve_ready: bool) {
        let generation = {
            let Ok(mut current) = self.inner.current_track.lock() else {
                return;
            };
            let (player_changed, matching_timeline_changed) = current
                .as_ref()
                .zip(track.as_ref())
                .map_or((false, false), |(current, next)| {
                    if current.key != next.key {
                        return (false, false);
                    }
                    (
                        current.player != next.player,
                        match (current.duration_ms, next.duration_ms) {
                            (Some(current), Some(next)) => {
                                current.abs_diff(next) > MAX_DURATION_DIFFERENCE_MS
                            }
                            (None, None) => false,
                            (None, Some(_)) | (Some(_), None) => true,
                        },
                    )
                });
            if *current == track && !player_changed && !matching_timeline_changed {
                // 身份未变化时仍更新最新时长，供后续本地缓存事件重新校验歌词。
                current.clone_from(&track);
                return;
            }
            current.clone_from(&track);
            // current_track 与 generation 必须在同一临界区内更新，避免并发媒体事件
            // 让旧歌曲获得更新的代数并覆盖刚切换的新歌曲。
            self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1
        };
        self.start_resolution(track, generation, preserve_ready, false);
    }

    /// 对已经登记为当前歌曲的描述启动一次解析，不再反向修改歌曲身份。
    fn start_resolution(
        &self,
        track: Option<TrackDescriptor>,
        generation: u64,
        preserve_ready: bool,
        cache_cleared: bool,
    ) {
        // 先确认代数仍然有效再重置链路：否则过期调用会把 trace_generation 改成旧值，
        // 让正在运行的新代数后续所有步骤都因代数不符而被丢弃。
        if !self.is_current_generation(generation) {
            return;
        }
        self.reset_resolution_trace(generation);
        let Some(track) = track else {
            self.cancel_resolution();
            self.publish_if_current(
                LyricsSnapshot::unavailable(None, "当前没有支持的歌曲"),
                generation,
            );
            return;
        };
        if !self.lyrics_enabled() {
            self.cancel_resolution();
            self.publish_if_current(
                LyricsSnapshot::unavailable(Some(track.key), "歌词显示已关闭"),
                generation,
            );
            return;
        }
        if track.duration_ms.is_none() {
            // 无时长有两种来源：播放器真的没给出时间线，或媒体层为了等时间线同步而先剥离了它
            // （切歌瞬间 SMTC 的标题与时间线可能不同步）。两者都无法解析，但都不能下结论——
            // 正常情况下确认过的快照马上就到，报“不可用”只会让每次切歌都多一条问题上报。
            log::debug!("当前曲目缺少有效时长，等待时间线确认: {}", track.key);
            self.cancel_resolution();
            self.publish_if_current(LyricsSnapshot::loading(track.key), generation);
            return;
        }
        let is_preview =
            players::is_preview_playback(&track, self.cache_path(track.player).as_deref())
                .inspect_err(|error| log::debug!("识别播放器试听状态失败: {error}"))
                .unwrap_or(false);
        if is_preview {
            self.cancel_resolution();
            self.publish_if_current(
                LyricsSnapshot::unavailable(Some(track.key), "试听播放不显示歌词"),
                generation,
            );
            return;
        }
        let keeps_current_content = preserve_ready
            && self.inner.runtime_state.read().is_ok_and(|state| {
                matches!(
                    state.snapshot.status,
                    LyricsStatus::Ready | LyricsStatus::Instrumental | LyricsStatus::NoLyrics
                ) && state.snapshot.track_key.as_ref() == Some(&track.key)
            });
        // 有任何可展示的缓存就先展示，过期与否都一样：重新解析期间让用户继续看上一版结果，
        // 比先空白再补上更稳。它同时仍作为本轮候选，拿到更好的来源会被正常替换。
        let cached = self.inner.cache.load(&track.key);
        let displayable_cache = cached
            .as_ref()
            .filter(|cached| is_cached_snapshot_displayable(&track, &cached.snapshot));
        if let Some(cached) = displayable_cache {
            self.publish_if_current_with_method(
                cached.snapshot.clone(),
                generation,
                LyricsResolutionMethod::ApplicationCache,
            );
        } else if !keeps_current_content {
            self.publish_if_current(LyricsSnapshot::loading(track.key.clone()), generation);
        }
        self.enqueue_resolution(track, generation, cached, cache_cleared);
    }

    /// 合并解析期间到达的新请求，始终只保留最新歌曲且最多运行一个主解析线程。
    fn enqueue_resolution(
        &self,
        track: TrackDescriptor,
        generation: u64,
        cached: Option<CacheLookup>,
        cache_cleared: bool,
    ) {
        let track_key = track.key.clone();
        let cancellation = Arc::new(AtomicBool::new(false));
        let should_start = {
            let Ok(mut resolver) = self.inner.resolver.lock() else {
                return;
            };
            if let Some(active) = resolver.active_cancellation.as_ref() {
                active.store(true, Ordering::Release);
            }
            if let Some(pending) = resolver.pending.as_ref() {
                pending.cancellation.store(true, Ordering::Release);
            }
            resolver.pending = Some(ResolutionRequest {
                track,
                generation,
                cancellation,
                cached,
                cache_cleared,
            });
            if resolver.running {
                false
            } else {
                resolver.running = true;
                true
            }
        };
        if !should_start {
            (self.inner.diagnostics_notifier)();
            return;
        }
        (self.inner.diagnostics_notifier)();
        let service = self.clone();
        if let Err(error) = thread::Builder::new()
            .name("lyrics-resolver".to_owned())
            .spawn(move || service.run_resolution_queue())
        {
            if let Ok(mut resolver) = self.inner.resolver.lock() {
                resolver.running = false;
                resolver.pending = None;
            }
            self.publish_if_current(
                LyricsSnapshot {
                    track_key: Some(track_key),
                    status: LyricsStatus::Error,
                    error_reason: Some(format!("启动歌词解析线程失败: {error}")),
                    ..LyricsSnapshot::default()
                },
                generation,
            );
            (self.inner.diagnostics_notifier)();
        }
    }

    /// 在没有替代任务时取消活动请求并丢弃尚未开始的旧请求。
    fn cancel_resolution(&self) {
        let Ok(mut resolver) = self.inner.resolver.lock() else {
            return;
        };
        if let Some(active) = resolver.active_cancellation.as_ref() {
            active.store(true, Ordering::Release);
        }
        if let Some(pending) = resolver.pending.take() {
            pending.cancellation.store(true, Ordering::Release);
        }
        drop(resolver);
        (self.inner.diagnostics_notifier)();
    }

    /// 串行消费最新请求；旧请求结束后跳过所有已经被更新请求覆盖的中间状态。
    fn run_resolution_queue(&self) {
        loop {
            let request = {
                let Ok(mut resolver) = self.inner.resolver.lock() else {
                    return;
                };
                let Some(request) = resolver.pending.take() else {
                    resolver.running = false;
                    resolver.active_cancellation = None;
                    drop(resolver);
                    (self.inner.diagnostics_notifier)();
                    return;
                };
                resolver.active_cancellation = Some(request.cancellation.clone());
                request
            };
            self.resolve_track_guarded(
                request.track,
                request.generation,
                request.cancellation,
                request.cached,
                request.cache_cleared,
            );
        }
    }

    /// 隔离单个平台或第三方库的非预期 panic，确保状态不会永久停留在加载中。
    fn resolve_track_guarded(
        &self,
        track: TrackDescriptor,
        generation: u64,
        cancellation: Arc<AtomicBool>,
        cached: Option<CacheLookup>,
        cache_cleared: bool,
    ) {
        let track_key = track.key.clone();
        self.begin_resolution_trace(generation, &track);
        let panicked = catch_unwind(AssertUnwindSafe(|| {
            self.resolve_track(track, generation, cancellation, cached, cache_cleared)
        }))
        .is_err();
        self.finish_resolution_trace(generation);
        if panicked {
            self.publish_if_current(
                LyricsSnapshot {
                    track_key: Some(track_key),
                    status: LyricsStatus::Error,
                    error_reason: Some("歌词解析器发生未预期异常".to_owned()),
                    ..LyricsSnapshot::default()
                },
                generation,
            );
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

    fn resolve_track(
        &self,
        track: TrackDescriptor,
        generation: u64,
        cancellation: Arc<AtomicBool>,
        cached: Option<CacheLookup>,
        cache_cleared: bool,
    ) {
        let deadline = ResolutionDeadline::new(cancellation);
        let preferences = self.preferences();
        let plan = ResolutionPlan::new(track.player, preferences.online_strategy);
        let mut candidates = Vec::new();
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

        // 缓存分支可能已经解析过当前播放器的本地来源（新鲜逐行的升级判定），此时本地阶段不必重复。
        let mut local_source_resolved = false;
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
                let mut upgraded_locally = false;
                let upgrade_started_at = Instant::now();
                local_source_resolved = true;
                match players::resolve_current_local(&track, self.cache_path(track.player)) {
                    Ok(LyricsLookupOutcome::Hit(local))
                        if is_acceptable_candidate(&track, &local.lines)
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
                        self.publish_resolution(&track, local, generation);
                        upgraded_locally = true;
                    }
                    Ok(LyricsLookupOutcome::Hit(local))
                        if !is_acceptable_candidate(&track, &local.lines) =>
                    {
                        self.record_resolution_step(
                            generation,
                            LyricsResolutionSite::LocalUpgrade,
                            LyricsResolutionOutcome::Error,
                            Some(format!(
                                "{} · {} ms",
                                timeline_rejection_reason(&track, &local.lines)
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
                    Err(LyricsError::Cancelled) => return,
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
                candidates.push(LyricsCandidate {
                    resolved: ResolvedLyrics {
                        source,
                        lines: cached.snapshot.lines,
                    },
                    resolution_method: LyricsResolutionMethod::ApplicationCache,
                });
            }
        }

        // 是否有来源技术性失败。用于把“来源暂时不可用”与“确认这首歌没有歌词”分开上报。
        let mut source_failed = false;
        // 已经展示了可用的逐行结果（缓存命中或本地命中）时，本轮在线只用于升级到逐字：
        // 换成另一个逐行只会让用户看到歌词无意义地跳一次，精度并没有提高。
        let mut upgrade_to_word_only = cached_line_displayed;

        let local_attempts = plan
            .local_attempts
            .iter()
            .filter(|attempt| !(local_source_resolved && attempt.player == track.player));
        for attempt in local_attempts {
            let execution = self.execute_attempt(*attempt, &track, &deadline);
            match self.record_attempt(execution, &track, generation, None) {
                RecordedAttempt::Candidate(candidate)
                    if candidate.resolved.source.player == track.player
                        && has_word_timing(&candidate.resolved.lines) =>
                {
                    self.publish_candidate(&track, candidate, generation);
                    return;
                }
                RecordedAttempt::Candidate(candidate) => {
                    // 本地逐行先发布：用户不必等在线阶段跑完才看到歌词，升级在后台继续。
                    // 但已展示逐行时（新鲜缓存）只有真正的提升才值得替换，否则歌词会无意义地跳一次。
                    if candidate.resolved.source.player == track.player
                        && improves_displayed(cached_line_displayed, &candidate)
                    {
                        upgrade_to_word_only = true;
                        self.publish_candidate(&track, candidate.clone(), generation);
                    }
                    candidates.push(candidate);
                }
                RecordedAttempt::Missed => {}
                RecordedAttempt::Failed => source_failed = true,
                RecordedAttempt::Cancelled => return,
            }
            if !self.is_current_generation(generation) {
                return;
            }
        }

        if !preferences.allow_online {
            match select_best_candidate(&track, candidates) {
                Some(candidate) if improves_displayed(upgrade_to_word_only, &candidate) => {
                    self.publish_candidate(&track, candidate, generation);
                }
                Some(_) => {}
                None => {
                    // 混搭快照（新标题 + 上一首时间线）会让所有来源同时未命中，此时下结论是错的：
                    // 先确认时长仍属于本曲，再留一个等待窗口，等修正后的描述符触发新一轮解析。
                    if !self.round_duration_is_current(&track)
                        || self.superseded_before_conclusion(generation)
                    {
                        log::debug!("本轮解析的输入可能已过期，放弃本次结论");
                    } else if let Some(snapshot) = stale_notice {
                        // 联网被策略禁止时，过期缓存仍是手上最有把握的结论。
                        self.store_and_publish_if_current(
                            snapshot,
                            generation,
                            LyricsResolutionMethod::ApplicationCache,
                        );
                    } else {
                        self.publish_no_lyrics(
                            &track.key,
                            generation,
                            source_failed,
                            "联网策略仅允许本地与缓存",
                            "本地与缓存歌词来源暂时不可用",
                        );
                    }
                }
            }
            return;
        }

        for (stage_index, stage) in plan.online_stages.iter().enumerate() {
            if stage.attempts.is_empty() {
                continue;
            }
            let executions = if stage.attempts.len() == 1 {
                vec![self.execute_attempt(stage.attempts[0], &track, &deadline)]
            } else {
                thread::scope(|scope| {
                    let service = self;
                    let track = &track;
                    let deadline = &deadline;
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
                match self.record_attempt(execution, &track, generation, stage.group) {
                    RecordedAttempt::Candidate(candidate)
                        if preferences.online_strategy
                            == LyricsOnlineStrategy::CurrentPlayerFirst
                            && stage_index == 0
                            && has_word_timing(&candidate.resolved.lines) =>
                    {
                        self.publish_candidate(&track, candidate, generation);
                        return;
                    }
                    RecordedAttempt::Candidate(candidate) => candidates.push(candidate),
                    RecordedAttempt::Missed => {}
                    RecordedAttempt::Failed => source_failed = true,
                    RecordedAttempt::Cancelled => return,
                }
            }
            if !self.is_current_generation(generation) {
                return;
            }
        }
        match select_best_candidate(&track, candidates) {
            Some(candidate) if improves_displayed(upgrade_to_word_only, &candidate) => {
                self.publish_candidate(&track, candidate, generation);
            }
            // 只升级不降级：已经展示的逐行保持不变，也不要让过期结论覆盖它。
            Some(_) => {}
            None => {
                // 同前：混搭快照下所有来源都会未命中，先把结论挂起，等修正后的新一轮接手。
                if !self.round_duration_is_current(&track)
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
                        source_failed,
                        "没有找到可靠歌词",
                        "歌词来源暂时不可用，请检查网络连接",
                    );
                }
            }
        }
    }

    /// 发布“本次没有拿到歌词”的收尾结论。
    ///
    /// 只有所有来源都正常执行并返回“没找到”时才说这首歌没有歌词；只要有来源技术性失败，
    /// 就按错误上报——两者在界面上都表现为没有歌词可显示，但前者重试无意义，后者往往只是
    /// 网络或平台暂时不可用，混为一谈会让用户以为这首歌根本没有歌词。
    fn publish_no_lyrics(
        &self,
        track_key: &str,
        generation: u64,
        source_failed: bool,
        miss_reason: &str,
        failure_reason: &str,
    ) {
        let snapshot = if source_failed {
            LyricsSnapshot {
                track_key: Some(track_key.to_owned()),
                status: LyricsStatus::Error,
                error_reason: Some(failure_reason.to_owned()),
                ..LyricsSnapshot::default()
            }
        } else {
            LyricsSnapshot::unavailable(Some(track_key.to_owned()), miss_reason)
        };
        self.store_and_publish_if_current(snapshot, generation, LyricsResolutionMethod::None);
    }

    /// 持久化解析结果，并仅在请求仍对应当前歌曲时发布，避免慢请求覆盖新歌曲。
    fn store_and_publish_if_current(
        &self,
        snapshot: LyricsSnapshot,
        generation: u64,
        resolution_method: LyricsResolutionMethod,
    ) {
        // 代数检查必须与歌曲身份更新互斥，否则旧任务可能覆盖新歌曲。
        if !self.current_generation_matches(generation) {
            return;
        }
        // 磁盘写入是本流程最慢的一步，移出歌曲身份锁，避免阻塞媒体监控线程更新当前歌曲。
        if let Err(error) = self.inner.cache.store(&snapshot, generation) {
            log::warn!("保存解析后歌词缓存失败: {error}");
        }
        // 写入期间可能已经切歌，发布前重新校验，避免把过期结果广播出去。
        self.publish_if_current_with_method(snapshot, generation, resolution_method);
    }

    /// 在歌曲身份锁内校验代数，供锁外工作的入口与出口复用。
    fn current_generation_matches(&self, generation: u64) -> bool {
        let Ok(_current) = self.inner.current_track.lock() else {
            return false;
        };
        self.is_current_generation(generation)
    }

    fn publish_if_current(&self, snapshot: LyricsSnapshot, generation: u64) {
        self.publish_if_current_with_method(snapshot, generation, LyricsResolutionMethod::None);
    }

    fn publish_if_current_with_method(
        &self,
        snapshot: LyricsSnapshot,
        generation: u64,
        resolution_method: LyricsResolutionMethod,
    ) {
        // 与歌曲身份更新使用同一把锁，使“检查代数 → 发布”不会被切歌事件穿插。
        let Ok(_current) = self.inner.current_track.lock() else {
            return;
        };
        if self.is_current_generation(generation) {
            self.publish(snapshot, resolution_method);
        }
    }

    fn is_current_generation(&self, generation: u64) -> bool {
        self.inner.generation.load(Ordering::Acquire) == generation
    }

    fn publish(&self, snapshot: LyricsSnapshot, resolution_method: LyricsResolutionMethod) {
        let snapshot_changed = self.inner.runtime_state.write().map_or(true, |mut state| {
            let changed = state.snapshot != snapshot;
            if changed {
                state.snapshot.clone_from(&snapshot);
            }
            state.resolution_method = resolution_method;
            changed
        });
        // 后台校验可能再次读出同一缓存；重复事件会让 WebView 重算歌词 DOM 并产生闪烁。
        if snapshot_changed {
            (self.inner.publisher)(&snapshot);
        }
        (self.inner.diagnostics_notifier)();
    }

    /// 读取一致的歌词偏好快照；锁损坏时回退到兼容旧版本的默认值。
    fn preferences(&self) -> LyricsPreferences {
        self.inner
            .preferences
            .read()
            .map_or_else(|_| LyricsPreferences::default(), |preferences| *preferences)
    }

    /// 歌词总开关是否开启；关闭后所有后台歌词工作都应停止。
    fn lyrics_enabled(&self) -> bool {
        self.preferences().enabled
    }

    fn cache_path(&self, player: MediaPlayer) -> Option<PathBuf> {
        self.inner
            .adapter_paths
            .read()
            .ok()
            .and_then(|paths| paths.get(&player).cloned().flatten())
    }

    /// 当前播放器匹配时先提升代数，再执行缓存变更，阻止旧任务回写失效结果。
    fn prepare_player_resolution(
        &self,
        player: MediaPlayer,
    ) -> Result<Option<(TrackDescriptor, u64)>, String> {
        let current = self
            .inner
            .current_track
            .lock()
            .map_err(|_| "当前歌曲状态不可用".to_owned())?;
        let Some(track) = current.as_ref().filter(|track| track.player == player) else {
            return Ok(None);
        };
        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        Ok(Some((track.clone(), generation)))
    }

    /// 仅在文件归属检查期间歌曲没有再次切换时预留解析代数。
    fn prepare_track_resolution(
        &self,
        player: MediaPlayer,
        expected_track_key: &str,
    ) -> Result<Option<(TrackDescriptor, u64)>, String> {
        let current = self
            .inner
            .current_track
            .lock()
            .map_err(|_| "当前歌曲状态不可用".to_owned())?;
        let Some(track) = current
            .as_ref()
            .filter(|track| track.player == player && track.key == expected_track_key)
        else {
            return Ok(None);
        };
        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        Ok(Some((track.clone(), generation)))
    }

    /// 启动可选的预留解析任务；非当前播放器的配置变化不会制造空任务。
    fn start_prepared_resolution(
        &self,
        pending: Option<(TrackDescriptor, u64)>,
        preserve_ready: bool,
        cache_cleared: bool,
    ) {
        if let Some((track, generation)) = pending {
            self.start_resolution(Some(track), generation, preserve_ready, cache_cleared);
        }
    }

    /// 保留当前歌曲身份并重新解析；缓存刷新可在解析期间继续显示已就绪歌词。
    fn force_resolve_current(
        &self,
        preserve_ready: bool,
        cache_cleared: bool,
    ) -> Result<(), String> {
        let (current, generation) = {
            let current = self
                .inner
                .current_track
                .lock()
                .map_err(|_| "当前歌曲状态不可用".to_owned())?;
            let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
            (current.clone(), generation)
        };
        self.start_resolution(current, generation, preserve_ready, cache_cleared);
        Ok(())
    }
}

fn is_allowed_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| ALLOWED_HTTPS_HOSTS.contains(&host))
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

/// 候选是否可以替换当前已展示的结果。
///
/// 已经展示逐行时只接受逐字：在线阶段拿到的另一个逐行精度并没有提高，替换只会让用户
/// 看到歌词无意义地跳一次。
fn improves_displayed(upgrade_to_word_only: bool, candidate: &LyricsCandidate) -> bool {
    !upgrade_to_word_only || has_word_timing(&candidate.resolved.lines)
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

fn display_path(path: &std::path::Path) -> String {
    let value = path.to_string_lossy();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}
