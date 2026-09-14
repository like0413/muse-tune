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

use notify::RecommendedWatcher;
use reqwest::{Url, blocking::Client, redirect};
use tauri::{Emitter, Manager, Runtime};

use crate::media::{MediaPlayer, MediaSessionSnapshot};

use super::{
    cache::{CacheLookup, ParsedLyricsCache},
    error::LyricsError,
    matcher::MAX_DURATION_DIFFERENCE_MS,
    model::{
        LyricsAdapterDiagnostics, LyricsCacheDiagnostics, LyricsDiagnostics, LyricsLookupMiss,
        LyricsLookupOutcome, LyricsOnlineStrategy, LyricsResolutionMethod, LyricsResolutionOutcome,
        LyricsResolutionStep, LyricsSnapshot, LyricsSnapshotDiagnostics, LyricsSourceKind,
        LyricsStatus, ResolvedLyrics, has_word_timing,
    },
    network::ResolutionDeadline,
    players,
    track::TrackDescriptor,
    watcher,
};

const LYRICS_CHANGED_EVENT: &str = "lyrics://changed";
const LYRICS_DIAGNOSTICS_CHANGED_EVENT: &str = "lyrics://diagnostics-changed";
const NETWORK_TIMEOUT: Duration = Duration::from_secs(8);
const ALLOWED_HTTPS_HOSTS: [&str; 4] = [
    "c.y.qq.com",
    "u.y.qq.com",
    "music.163.com",
    "beta-luna.douyin.com",
];

type SnapshotPublisher = dyn Fn(&LyricsSnapshot) + Send + Sync;
type DiagnosticsNotifier = dyn Fn() + Send + Sync;
type LyricsResolutionResult = Result<LyricsLookupOutcome, LyricsError>;
type LabeledLyricsResolutionResult<'a> = (&'a str, Option<LyricsResolutionResult>);

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
    watchers: Mutex<HashMap<MediaPlayer, RecommendedWatcher>>,
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
    fn default() -> Self {
        Self {
            enabled: true,
            allow_online: true,
            online_strategy: LyricsOnlineStrategy::default(),
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
}

#[derive(Clone, Copy)]
enum TimelineValidation {
    Plausible,
    DurationMismatch {
        track_duration_ms: u64,
        latest_start_ms: u64,
        latest_end_ms: u64,
    },
    Invalid,
}

#[derive(Default)]
struct LyricsRuntimeState {
    snapshot: LyricsSnapshot,
    resolution_method: LyricsResolutionMethod,
    trace_generation: u64,
    resolution_started_at: Option<Instant>,
    resolution_duration_ms: Option<u64>,
    resolution_steps: Vec<LyricsResolutionStep>,
}

struct LyricsCandidate {
    resolved: ResolvedLyrics,
    resolution_method: LyricsResolutionMethod,
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
                resolver: Mutex::new(ResolverState::default()),
            }),
        };
        service.refresh_watchers();
        service.start_registry_watcher();
        Ok(service)
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
        let (snapshot, resolution_method, resolution_duration_ms, resolution_steps) =
            self.inner.runtime_state.read().map_or_else(
                |_| {
                    (
                        LyricsSnapshotDiagnostics::default(),
                        LyricsResolutionMethod::None,
                        None,
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
                        state.resolution_steps.clone(),
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
            resolution_steps,
            cache: self
                .inner
                .cache
                .diagnostics(current_track.as_ref().map(|track| track.key.as_str())),
            adapters,
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
    pub fn clear_cache(&self) -> Result<(), io::Error> {
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
        self.start_resolution(Some(track), generation, false);
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
            self.publish_if_current(
                LyricsSnapshot::unavailable(track_key, "歌词显示已关闭"),
                generation,
            );
            return Ok(());
        }
        if enabled_changed || online_changed {
            self.force_resolve_current(online_changed && allow_online)
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
        self.start_resolution(track, generation, preserve_ready);
    }

    /// 对已经登记为当前歌曲的描述启动一次解析，不再反向修改歌曲身份。
    fn start_resolution(
        &self,
        track: Option<TrackDescriptor>,
        generation: u64,
        preserve_ready: bool,
    ) {
        self.reset_resolution_trace(generation);
        if !self.is_current_generation(generation) {
            return;
        }
        let Some(track) = track else {
            self.cancel_resolution();
            self.publish_if_current(
                LyricsSnapshot::unavailable(None, "当前没有支持的歌曲"),
                generation,
            );
            return;
        };
        if !self.preferences().enabled {
            self.cancel_resolution();
            self.publish_if_current(
                LyricsSnapshot::unavailable(Some(track.key), "歌词显示已关闭"),
                generation,
            );
            return;
        }
        if track.duration_ms.is_none() {
            self.cancel_resolution();
            self.publish_if_current(
                LyricsSnapshot::unavailable(Some(track.key), "当前播放器未提供有效播放时间线"),
                generation,
            );
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
                    LyricsStatus::Ready | LyricsStatus::Instrumental
                ) && state.snapshot.track_key.as_ref() == Some(&track.key)
            });
        // 有效歌词或纯音乐结论直接展示，避免重复解析造成内容层闪烁。
        let cached = self.inner.cache.load(&track.key);
        let displayable_cache = cached.as_ref().filter(|cached| {
            cached.is_fresh && is_cached_snapshot_displayable(&track, &cached.snapshot)
        });
        if let Some(cached) = displayable_cache {
            self.publish_if_current_with_method(
                cached.snapshot.clone(),
                generation,
                LyricsResolutionMethod::ApplicationCache,
            );
        } else if !keeps_current_content {
            self.publish_if_current(LyricsSnapshot::loading(track.key.clone()), generation);
        }
        self.enqueue_resolution(track, generation, cached);
    }

    /// 合并解析期间到达的新请求，始终只保留最新歌曲且最多运行一个主解析线程。
    fn enqueue_resolution(
        &self,
        track: TrackDescriptor,
        generation: u64,
        cached: Option<CacheLookup>,
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
    ) {
        let track_key = track.key.clone();
        self.begin_resolution_trace(generation);
        let panicked = catch_unwind(AssertUnwindSafe(|| {
            self.resolve_track(track, generation, cancellation, cached)
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

    fn resolve_track(
        &self,
        track: TrackDescriptor,
        generation: u64,
        cancellation: Arc<AtomicBool>,
        cached: Option<CacheLookup>,
    ) {
        let deadline = ResolutionDeadline::new(cancellation);
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
            "Muse Tune 缓存",
            if cached.is_some() {
                LyricsResolutionOutcome::Hit
            } else {
                LyricsResolutionOutcome::Miss
            },
            Some(cached.as_ref().map_or_else(
                || "解析开始时无应用缓存".to_owned(),
                |cached| match (cached.snapshot.status, cache_timeline_validation) {
                    (LyricsStatus::Instrumental, _) if cached.is_fresh => {
                        "已确认纯音乐，有效期内".to_owned()
                    }
                    (LyricsStatus::Instrumental, _) => "已确认纯音乐，已过期，重新确认".to_owned(),
                    (_, Some(TimelineValidation::Plausible)) if cached.is_fresh => {
                        "有效期内".to_owned()
                    }
                    (_, Some(TimelineValidation::Plausible)) => "已过期，作为兜底候选".to_owned(),
                    (
                        _,
                        Some(TimelineValidation::DurationMismatch {
                            track_duration_ms,
                            latest_start_ms,
                            latest_end_ms,
                        }),
                    ) => format!(
                        "已读取；播放器时长 {}，歌词末行开始 {}、结束 {}；采用有效缓存",
                        format_milliseconds(track_duration_ms),
                        format_milliseconds(latest_start_ms),
                        format_milliseconds(latest_end_ms),
                    ),
                    (_, Some(TimelineValidation::Invalid) | None) => {
                        "已读取，但缓存状态或时间轴结构无效".to_owned()
                    }
                },
            )),
        );
        if let Some(cached) = cached {
            if cached_snapshot_displayable && cached.is_fresh {
                let should_check_local = cached.snapshot.status == LyricsStatus::Ready
                    && cached.snapshot.precision != Some(super::model::LyricsPrecision::Word);
                self.publish_if_current_with_method(
                    cached.snapshot.clone(),
                    generation,
                    LyricsResolutionMethod::ApplicationCache,
                );
                if !should_check_local || !self.is_current_generation(generation) {
                    return;
                }

                // 已有缓存必须先展示；本地精度升级属于增强路径，不能阻塞首屏歌词。
                let upgrade_started_at = Instant::now();
                match players::resolve_current_local(&track, self.cache_path(track.player)) {
                    Ok(LyricsLookupOutcome::Hit(local))
                        if is_plausible_timeline(&track, &local.lines)
                            && (has_word_timing(&local.lines)
                                || auxiliary_content_count(&local.lines)
                                    > auxiliary_content_count(&cached.snapshot.lines)) =>
                    {
                        self.record_resolution_step(
                            generation,
                            "后台本地歌词升级",
                            LyricsResolutionOutcome::Hit,
                            Some(format!(
                                "发现更高精度或辅助内容更完整的本地歌词 · {} ms",
                                duration_millis(upgrade_started_at.elapsed()),
                            )),
                        );
                        self.publish_resolution(&track, local, generation);
                    }
                    Ok(LyricsLookupOutcome::Hit(local))
                        if !is_plausible_timeline(&track, &local.lines) =>
                    {
                        self.record_resolution_step(
                            generation,
                            "后台本地歌词升级",
                            LyricsResolutionOutcome::Error,
                            Some(format!(
                                "结果时间轴超出歌曲有效范围 · {} ms",
                                duration_millis(upgrade_started_at.elapsed()),
                            )),
                        );
                    }
                    Ok(LyricsLookupOutcome::Hit(_)) => self.record_resolution_step(
                        generation,
                        "后台本地歌词升级",
                        LyricsResolutionOutcome::Miss,
                        Some(format!(
                            "本地歌词未提供更高精度或更多辅助内容 · {} ms",
                            duration_millis(upgrade_started_at.elapsed()),
                        )),
                    ),
                    Ok(LyricsLookupOutcome::Miss(reason)) => self.record_resolution_step(
                        generation,
                        "后台本地歌词升级",
                        LyricsResolutionOutcome::Miss,
                        Some(format!(
                            "{} · {} ms",
                            lookup_miss_detail(reason),
                            duration_millis(upgrade_started_at.elapsed()),
                        )),
                    ),
                    Ok(LyricsLookupOutcome::Unsupported) => self.record_resolution_step(
                        generation,
                        "后台本地歌词升级",
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
                            "后台本地歌词升级",
                            LyricsResolutionOutcome::Error,
                            Some(format!(
                                "{error} · {} ms",
                                duration_millis(upgrade_started_at.elapsed()),
                            )),
                        );
                        log::debug!("检查播放器本地歌词升级失败: {error}");
                    }
                }
                return;
            }
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

        let path = self.cache_path(track.player);
        let current_local_result = players::resolve_current_local(&track, path.clone());
        match current_local_result {
            Ok(LyricsLookupOutcome::Hit(resolved))
                if is_plausible_timeline(&track, &resolved.lines) =>
            {
                self.record_resolution_step(
                    generation,
                    "当前播放器本地",
                    LyricsResolutionOutcome::Hit,
                    Some(source_summary(&resolved)),
                );
                if has_word_timing(&resolved.lines) {
                    self.publish_resolution(&track, resolved, generation);
                    return;
                }
                candidates.push(candidate_from_source(resolved));
            }
            Ok(LyricsLookupOutcome::Hit(_)) => {
                self.record_resolution_step(
                    generation,
                    "当前播放器本地",
                    LyricsResolutionOutcome::Error,
                    Some("歌词时间轴超出歌曲有效范围".to_owned()),
                );
                log::warn!("当前播放器本地歌词时间轴超出歌曲有效范围");
            }
            Ok(LyricsLookupOutcome::Miss(reason)) => self.record_resolution_step(
                generation,
                "当前播放器本地",
                LyricsResolutionOutcome::Miss,
                Some(lookup_miss_detail(reason).to_owned()),
            ),
            Ok(LyricsLookupOutcome::Unsupported) => self.record_resolution_step(
                generation,
                "当前播放器本地",
                LyricsResolutionOutcome::Miss,
                Some("当前播放器不支持本地歌词".to_owned()),
            ),
            Err(LyricsError::Cancelled) => return,
            Err(error) => {
                self.record_resolution_step(
                    generation,
                    "当前播放器本地",
                    LyricsResolutionOutcome::Error,
                    Some(error.to_string()),
                );
                log::warn!("当前播放器本地歌词适配器失败: {error}");
            }
        }
        if !self.is_current_generation(generation) {
            return;
        }

        let qq_cache_path = self.cache_path(MediaPlayer::QqMusic);
        if track.player != MediaPlayer::QqMusic {
            match players::resolve_local_for(MediaPlayer::QqMusic, &track, qq_cache_path) {
                Ok(LyricsLookupOutcome::Hit(resolved))
                    if is_plausible_timeline(&track, &resolved.lines) =>
                {
                    self.record_resolution_step(
                        generation,
                        "QQ 本地兜底",
                        LyricsResolutionOutcome::Hit,
                        Some(source_summary(&resolved)),
                    );
                    if has_word_timing(&resolved.lines) {
                        self.publish_resolution(&track, resolved, generation);
                        return;
                    }
                    candidates.push(candidate_from_source(resolved));
                }
                Ok(LyricsLookupOutcome::Hit(_)) => self.record_resolution_step(
                    generation,
                    "QQ 本地兜底",
                    LyricsResolutionOutcome::Error,
                    Some("歌词时间轴超出歌曲有效范围".to_owned()),
                ),
                Ok(LyricsLookupOutcome::Miss(reason)) => self.record_resolution_step(
                    generation,
                    "QQ 本地兜底",
                    LyricsResolutionOutcome::Miss,
                    Some(lookup_miss_detail(reason).to_owned()),
                ),
                Ok(LyricsLookupOutcome::Unsupported) => {}
                Err(LyricsError::Cancelled) => return,
                Err(error) => {
                    self.record_resolution_step(
                        generation,
                        "QQ 本地兜底",
                        LyricsResolutionOutcome::Error,
                        Some(error.to_string()),
                    );
                    log::warn!("QQ 本地歌词兜底失败: {error}");
                }
            }
        }
        if !self.is_current_generation(generation) {
            return;
        }

        let preferences = self.preferences();
        if !preferences.allow_online {
            if let Some(candidate) = select_best_candidate(&track, candidates) {
                self.publish_candidate(&track, candidate, generation);
            } else {
                self.store_and_publish_if_current(
                    LyricsSnapshot::unavailable(
                        Some(track.key.clone()),
                        "联网策略仅允许本地与缓存",
                    ),
                    generation,
                    LyricsResolutionMethod::None,
                );
            }
            return;
        }

        match preferences.online_strategy {
            LyricsOnlineStrategy::Parallel => {
                let (current_online_result, qq_online_result, netease_result) =
                    thread::scope(|scope| {
                        let current_online = scope.spawn(|| {
                            players::resolve_current_online(
                                &track,
                                path,
                                &self.inner.client,
                                &deadline,
                            )
                        });
                        let qq_online = (track.player != MediaPlayer::QqMusic).then(|| {
                            scope.spawn(|| {
                                players::resolve_online_for(
                                    MediaPlayer::QqMusic,
                                    &track,
                                    None,
                                    &self.inner.client,
                                    &deadline,
                                )
                            })
                        });
                        let netease = (track.player != MediaPlayer::NeteaseCloudMusic).then(|| {
                            scope.spawn(|| {
                                players::resolve_online_for(
                                    MediaPlayer::NeteaseCloudMusic,
                                    &track,
                                    None,
                                    &self.inner.client,
                                    &deadline,
                                )
                            })
                        });
                        (
                            current_online.join().ok(),
                            qq_online.and_then(|handle| handle.join().ok()),
                            netease.and_then(|handle| handle.join().ok()),
                        )
                    });
                let online_results = [
                    ("当前播放器在线", current_online_result),
                    ("QQ 在线兜底", qq_online_result),
                    ("网易云在线兜底", netease_result),
                ];
                let parallel_group = (online_results
                    .iter()
                    .filter(|(_, result)| result.is_some())
                    .count()
                    > 1)
                .then_some("并行在线查询");
                self.collect_online_results(
                    generation,
                    parallel_group,
                    online_results,
                    &mut candidates,
                );
            }
            LyricsOnlineStrategy::CurrentPlayerFirst => {
                let current_result =
                    players::resolve_current_online(&track, path, &self.inner.client, &deadline);
                let (outcome, detail) = summarize_resolution_result(&current_result);
                self.record_resolution_step(generation, "当前播放器在线优先", outcome, detail);
                match current_result {
                    Ok(LyricsLookupOutcome::Hit(resolved))
                        if is_plausible_timeline(&track, &resolved.lines)
                            && has_word_timing(&resolved.lines) =>
                    {
                        self.publish_resolution(&track, resolved, generation);
                        return;
                    }
                    Ok(LyricsLookupOutcome::Hit(resolved)) => {
                        candidates.push(candidate_from_source(resolved));
                    }
                    Ok(LyricsLookupOutcome::Unsupported | LyricsLookupOutcome::Miss(_)) => {}
                    Err(LyricsError::Cancelled) => return,
                    Err(error) => log::warn!("当前播放器在线歌词适配器失败: {error}"),
                }

                let (qq_online_result, netease_result) = thread::scope(|scope| {
                    let qq_online = (track.player != MediaPlayer::QqMusic).then(|| {
                        scope.spawn(|| {
                            players::resolve_online_for(
                                MediaPlayer::QqMusic,
                                &track,
                                None,
                                &self.inner.client,
                                &deadline,
                            )
                        })
                    });
                    let netease = (track.player != MediaPlayer::NeteaseCloudMusic).then(|| {
                        scope.spawn(|| {
                            players::resolve_online_for(
                                MediaPlayer::NeteaseCloudMusic,
                                &track,
                                None,
                                &self.inner.client,
                                &deadline,
                            )
                        })
                    });
                    (
                        qq_online.and_then(|handle| handle.join().ok()),
                        netease.and_then(|handle| handle.join().ok()),
                    )
                });
                let fallback_results = [
                    ("QQ 在线兜底", qq_online_result),
                    ("网易云在线兜底", netease_result),
                ];
                let parallel_group = (fallback_results
                    .iter()
                    .filter(|(_, result)| result.is_some())
                    .count()
                    > 1)
                .then_some("并行在线兜底");
                self.collect_online_results(
                    generation,
                    parallel_group,
                    fallback_results,
                    &mut candidates,
                );
            }
        }
        if let Some(candidate) = select_best_candidate(&track, candidates) {
            self.publish_candidate(&track, candidate, generation);
        } else {
            self.store_and_publish_if_current(
                LyricsSnapshot::unavailable(Some(track.key.clone()), "没有找到可靠歌词"),
                generation,
                LyricsResolutionMethod::None,
            );
        }
    }

    /// 保存最终歌词并仅在歌曲代数仍有效时发布。
    fn publish_resolution(
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
    fn publish_candidate(
        &self,
        track: &TrackDescriptor,
        candidate: LyricsCandidate,
        generation: u64,
    ) {
        let snapshot = LyricsSnapshot::from_resolved(track.key.clone(), candidate.resolved);
        self.store_and_publish_if_current(snapshot, generation, candidate.resolution_method);
    }

    /// 持久化解析结果，并仅在请求仍对应当前歌曲时发布，避免慢请求覆盖新歌曲。
    fn store_and_publish_if_current(
        &self,
        snapshot: LyricsSnapshot,
        generation: u64,
        resolution_method: LyricsResolutionMethod,
    ) {
        // 代数检查、缓存写入和发布必须与歌曲身份更新互斥，否则旧任务可能覆盖新歌曲。
        let Ok(_current) = self.inner.current_track.lock() else {
            return;
        };
        if !self.is_current_generation(generation) {
            return;
        }
        if let Err(error) = self.inner.cache.store(&snapshot) {
            log::warn!("保存解析后歌词缓存失败: {error}");
        }
        self.publish(snapshot, resolution_method);
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

    fn begin_resolution_trace(&self, generation: u64) {
        if let Ok(mut state) = self.inner.runtime_state.write() {
            state.trace_generation = generation;
            state.resolution_started_at = Some(Instant::now());
            state.resolution_duration_ms = None;
            state.resolution_steps.clear();
        }
    }

    /// 新解析代数一经接受就清除旧歌曲链路，提前返回时不展示历史结果。
    fn reset_resolution_trace(&self, generation: u64) {
        if let Ok(mut state) = self.inner.runtime_state.write() {
            state.trace_generation = generation;
            state.resolution_started_at = None;
            state.resolution_duration_ms = None;
            state.resolution_steps.clear();
        }
    }

    fn record_resolution_step(
        &self,
        generation: u64,
        label: &str,
        outcome: LyricsResolutionOutcome,
        detail: Option<String>,
    ) {
        self.record_resolution_step_in_group(generation, None, label, outcome, detail);
    }

    /// 记录一次在线查询，并在诊断数据中保留其并发阶段。
    fn record_resolution_step_in_group(
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

    /// 汇总同一在线阶段的结果，统一写入诊断链路与候选集合。
    fn collect_online_results<const N: usize>(
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

    /// 读取一致的歌词偏好快照；锁损坏时回退到兼容旧版本的默认值。
    fn preferences(&self) -> LyricsPreferences {
        self.inner
            .preferences
            .read()
            .map_or_else(|_| LyricsPreferences::default(), |preferences| *preferences)
    }

    fn finish_resolution_trace(&self, generation: u64) {
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

    /// 按播放器分别重建非递归监听器，避免其他播放器的写入刷新当前歌词。
    fn refresh_watchers(&self) {
        let next_paths = players::supported_players()
            .map(|player| (player, players::automatic_cache_path(player)))
            .collect::<HashMap<_, _>>();
        if let Ok(mut current) = self.inner.adapter_paths.write() {
            current.clone_from(&next_paths);
        }
        let mut next_watchers = HashMap::new();
        for player in players::supported_players() {
            let cache_path = next_paths.get(&player).cloned().flatten();
            let paths = players::watch_paths_for(player, cache_path.as_deref());
            let weak_inner = Arc::downgrade(&self.inner);
            let callback: Arc<dyn Fn(Vec<PathBuf>) + Send + Sync> = Arc::new(move |paths| {
                let Some(inner) = weak_inner.upgrade() else {
                    return;
                };
                let service = LyricsService { inner };
                let configuration_changed = players::configuration_changed(player, &paths);
                let result = if configuration_changed {
                    service.handle_configuration_change(player)
                } else {
                    service.handle_cache_content_change(player, &paths)
                };
                if let Err(error) = result {
                    log::warn!("响应 {player:?} 歌词缓存变化失败: {error}");
                }
            });
            match watcher::create(paths, callback) {
                Ok(Some(watcher)) => {
                    next_watchers.insert(player, watcher);
                }
                Ok(None) => {}
                Err(error) => log::warn!("建立 {player:?} 歌词缓存监听失败: {error}"),
            }
        }
        if let Ok(mut current) = self.inner.watchers.lock() {
            *current = next_watchers;
        }
    }

    /// 启动适配器声明的原生缓存路径设置监听；回调只持有服务的弱引用。
    fn start_registry_watcher(&self) {
        let weak_inner = Arc::downgrade(&self.inner);
        players::watch_registry_settings(Arc::new(move |player| {
            let Some(inner) = weak_inner.upgrade() else {
                return;
            };
            let service = LyricsService { inner };
            if let Err(error) = service.handle_configuration_change(player) {
                log::warn!("响应 {player:?} 缓存目录设置变化失败: {error}");
            }
        }));
    }

    /// 播放器目录配置变化属于低频事件，需要重建监听并淘汰旧来源结果。
    fn handle_configuration_change(&self, player: MediaPlayer) -> Result<(), String> {
        let pending = self.prepare_player_resolution(player)?;
        self.refresh_watchers();
        if let Err(error) = self.inner.cache.clear_local_source(player) {
            log::warn!("清理播放器旧本地歌词缓存失败: {error}");
        }
        self.start_prepared_resolution(pending, true);
        Ok(())
    }

    /// 只淘汰当前播放器、当前歌曲的结果，其他播放器写缓存时不做任何工作。
    fn handle_cache_content_change(
        &self,
        player: MediaPlayer,
        paths: &[PathBuf],
    ) -> Result<(), String> {
        let cache_path = self.cache_path(player);
        // notify 事件可能早于 Windows 目录修改时间更新，先按事件事实淘汰旧索引。
        players::invalidate_local_index(player, cache_path.as_deref());
        let watched_paths = players::watch_paths_for(player, cache_path.as_deref());
        let watch_root_changed = paths.iter().any(|changed| {
            watched_paths
                .iter()
                .any(|watched| watcher::paths_equivalent(changed, watched))
        });
        if watch_root_changed {
            self.refresh_watchers();
        }
        let track = self
            .inner
            .current_track
            .lock()
            .map_err(|_| "当前歌曲状态不可用".to_owned())?
            .as_ref()
            .filter(|track| track.player == player)
            .cloned();
        let Some(track) = track else {
            return Ok(());
        };
        let (current_source, current_has_word_timing) = self
            .inner
            .runtime_state
            .read()
            .ok()
            .filter(|state| state.snapshot.track_key.as_ref() == Some(&track.key))
            .map_or((None, false), |state| {
                (
                    state.snapshot.source.clone(),
                    has_word_timing(&state.snapshot.lines),
                )
            });
        if !watch_root_changed
            && !players::changed_paths_affect_track(
                &track,
                cache_path.as_deref(),
                paths,
                current_source.as_ref(),
                current_has_word_timing,
            )
        {
            return Ok(());
        }
        let current_uses_player_local = current_source.as_ref().is_some_and(|source| {
            source.player == player && source.kind == LyricsSourceKind::Local
        });
        let should_compare_local = current_uses_player_local || current_has_word_timing;
        let local_after_change = should_compare_local
            .then(|| players::resolve_current_local(&track, cache_path.clone()));
        if let Some(Err(error)) = local_after_change.as_ref() {
            log::debug!("检查播放器本地歌词变化失败: {error}");
        }
        if current_uses_player_local
            && local_after_change
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .and_then(lookup_hit)
                .is_some_and(|local| self.current_snapshot_matches(&track.key, local))
        {
            return Ok(());
        }
        // 播放器切歌会改写队列和行级歌词缓存，不能因此淘汰其他来源的逐字结果。
        if current_has_word_timing && !current_uses_player_local {
            let local_can_replace_word_timing = local_after_change
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .and_then(lookup_hit)
                .is_some_and(|local| {
                    is_plausible_timeline(&track, &local.lines) && has_word_timing(&local.lines)
                });
            if !local_can_replace_word_timing {
                return Ok(());
            }
        }
        let pending = self.prepare_track_resolution(player, &track.key)?;
        let Some((track, generation)) = pending else {
            return Ok(());
        };
        if let Err(error) = self.inner.cache.remove(&track.key) {
            log::warn!("清理当前歌曲解析缓存失败: {error}");
        }
        self.start_resolution(Some(track), generation, true);
        Ok(())
    }

    /// 比较播放器事件后的本地结果与当前快照，忽略仅触碰文件但内容未变的事件。
    fn current_snapshot_matches(&self, track_key: &str, local: &ResolvedLyrics) -> bool {
        self.inner.runtime_state.read().is_ok_and(|state| {
            state.snapshot.track_key.as_deref() == Some(track_key)
                && state.snapshot.source.as_ref() == Some(&local.source)
                && state.snapshot.lines == local.lines
        })
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
    ) {
        if let Some((track, generation)) = pending {
            self.start_resolution(Some(track), generation, preserve_ready);
        }
    }

    /// 保留当前歌曲身份并重新解析；缓存刷新可在解析期间继续显示已就绪歌词。
    fn force_resolve_current(&self, preserve_ready: bool) -> Result<(), String> {
        let (current, generation) = {
            let current = self
                .inner
                .current_track
                .lock()
                .map_err(|_| "当前歌曲状态不可用".to_owned())?;
            let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
            (current.clone(), generation)
        };
        self.start_resolution(current, generation, preserve_ready);
        Ok(())
    }
}

fn is_allowed_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| ALLOWED_HTTPS_HOSTS.contains(&host))
}

/// 跨平台候选先比较时间精度，再保持当前平台和本地来源优先。
fn select_best_candidate(
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
fn auxiliary_content_count(lines: &[super::model::LyricLine]) -> usize {
    lines
        .iter()
        .map(|line| {
            usize::from(line.translation.is_some()) + usize::from(line.romanization.is_some())
        })
        .sum()
}

fn candidate_from_source(resolved: ResolvedLyrics) -> LyricsCandidate {
    LyricsCandidate {
        resolution_method: method_from_source(&resolved),
        resolved,
    }
}

/// 只借用命中值，供文件事件比较路径使用。
fn lookup_hit(outcome: &LyricsLookupOutcome) -> Option<&ResolvedLyrics> {
    match outcome {
        LyricsLookupOutcome::Hit(resolved) => Some(resolved),
        LyricsLookupOutcome::Unsupported | LyricsLookupOutcome::Miss(_) => None,
    }
}

fn method_from_source(resolved: &ResolvedLyrics) -> LyricsResolutionMethod {
    match resolved.source.kind {
        LyricsSourceKind::Local => LyricsResolutionMethod::PlayerLocal,
        LyricsSourceKind::Online => LyricsResolutionMethod::Online,
    }
}

fn summarize_resolution_result(
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
fn lookup_miss_detail(reason: LyricsLookupMiss) -> &'static str {
    match reason {
        LyricsLookupMiss::DataUnavailable => "解析所需的本地数据不可用",
        LyricsLookupMiss::NoReliableLyrics => "没有找到可靠歌词",
    }
}

fn source_summary(resolved: &ResolvedLyrics) -> String {
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

fn display_path(path: &std::path::Path) -> String {
    let value = path.to_string_lossy();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// 区分播放器暂时报短的时长与歌词数据损坏，供缓存展示和新候选校验采用不同策略。
fn validate_timeline(
    track: &TrackDescriptor,
    lines: &[super::model::LyricLine],
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
fn is_cached_snapshot_displayable(track: &TrackDescriptor, snapshot: &LyricsSnapshot) -> bool {
    match snapshot.status {
        LyricsStatus::Instrumental => true,
        LyricsStatus::Ready => {
            is_cached_timeline_displayable(validate_timeline(track, &snapshot.lines))
        }
        LyricsStatus::Loading | LyricsStatus::Unavailable | LyricsStatus::Error => false,
    }
}

/// 拒绝明显超出歌曲时长或顺序倒退的解析结果，避免错误候选进入长期缓存。
fn is_plausible_timeline(track: &TrackDescriptor, lines: &[super::model::LyricLine]) -> bool {
    matches!(
        validate_timeline(track, lines),
        TimelineValidation::Plausible
    )
}

/// 以诊断友好的秒数显示毫秒时间点。
fn format_milliseconds(milliseconds: u64) -> String {
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
    use crate::media::MediaPlayer;

    use super::{TimelineValidation, TrackDescriptor, is_plausible_timeline, validate_timeline};
    use crate::lyrics::model::{LyricLine, LyricWord};

    fn track() -> TrackDescriptor {
        TrackDescriptor {
            key: "track".to_owned(),
            player: MediaPlayer::QqMusic,
            title: "歌曲".to_owned(),
            artists: vec!["歌手".to_owned()],
            duration_ms: Some(180_000),
        }
    }

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
            &[line(2_000), line(1_000)]
        ));
    }

    #[test]
    fn timeline_accepts_ordered_lines_inside_duration() {
        assert!(is_plausible_timeline(&track(), &[line(1_000), line(2_000)]));
    }
}
