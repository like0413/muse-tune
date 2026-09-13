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
    cache::ParsedLyricsCache,
    error::LyricsError,
    model::{
        LyricsAdapterDiagnostics, LyricsCacheDiagnostics, LyricsDiagnostics,
        LyricsResolutionMethod, LyricsResolutionOutcome, LyricsResolutionStep, LyricsSnapshot,
        LyricsSnapshotDiagnostics, LyricsSourceKind, LyricsStatus, ResolvedLyrics, has_word_timing,
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
    enabled: AtomicBool,
    allow_online: AtomicBool,
    publisher: Arc<SnapshotPublisher>,
    diagnostics_notifier: Arc<DiagnosticsNotifier>,
    runtime_state: RwLock<LyricsRuntimeState>,
    adapter_paths: RwLock<HashMap<MediaPlayer, Option<PathBuf>>>,
    watchers: Mutex<HashMap<MediaPlayer, RecommendedWatcher>>,
    resolver: Mutex<ResolverState>,
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
        let (enabled, allow_online) = super::settings::restore_lyrics_preferences(app);
        let service = Self {
            inner: Arc::new(LyricsServiceInner {
                cache,
                client,
                current_track: Mutex::new(None),
                generation: AtomicU64::new(0),
                enabled: AtomicBool::new(enabled),
                allow_online: AtomicBool::new(allow_online),
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
                players::SUPPORTED_PLAYERS
                    .into_iter()
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
        LyricsDiagnostics {
            snapshot,
            current_player,
            enabled: self.inner.enabled.load(Ordering::Acquire),
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

    /// 原子更新歌词开关与联网能力，并只触发一次必要的重新解析。
    pub fn set_preferences(&self, enabled: bool, allow_online: bool) -> Result<(), String> {
        let enabled_changed = self.inner.enabled.swap(enabled, Ordering::AcqRel) != enabled;
        let online_changed =
            self.inner.allow_online.swap(allow_online, Ordering::AcqRel) != allow_online;
        if !enabled_changed && !online_changed {
            return Ok(());
        }
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
        self.force_resolve_current(online_changed && allow_online)
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
            let timeline_availability_changed =
                current
                    .as_ref()
                    .zip(track.as_ref())
                    .is_some_and(|(current, next)| {
                        current.key == next.key
                            && current.duration_ms.is_some() != next.duration_ms.is_some()
                    });
            if *current == track && !timeline_availability_changed {
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
        if !self.inner.enabled.load(Ordering::Acquire) {
            self.cancel_resolution();
            self.publish_if_current(
                LyricsSnapshot::unavailable(Some(track.key), "歌词显示已关闭"),
                generation,
            );
            return;
        }
        // 没有有效 GSMTC 时间线时无法可靠同步歌词，必须在读缓存和启动解析前回退。
        if track.duration_ms.is_none() {
            self.cancel_resolution();
            self.publish_if_current(
                LyricsSnapshot::unavailable(Some(track.key), "当前播放器未提供有效播放时间线"),
                generation,
            );
            return;
        }
        let keeps_current_ready = preserve_ready
            && self.inner.runtime_state.read().is_ok_and(|state| {
                state.snapshot.status == LyricsStatus::Ready
                    && state.snapshot.track_key.as_ref() == Some(&track.key)
            });
        if !keeps_current_ready {
            self.publish_if_current(LyricsSnapshot::loading(track.key.clone()), generation);
        }
        self.enqueue_resolution(track, generation);
    }

    /// 合并解析期间到达的新请求，始终只保留最新歌曲且最多运行一个主解析线程。
    fn enqueue_resolution(&self, track: TrackDescriptor, generation: u64) {
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
            self.resolve_track_guarded(request.track, request.generation, request.cancellation);
        }
    }

    /// 隔离单个平台或第三方库的非预期 panic，确保状态不会永久停留在加载中。
    fn resolve_track_guarded(
        &self,
        track: TrackDescriptor,
        generation: u64,
        cancellation: Arc<AtomicBool>,
    ) {
        let track_key = track.key.clone();
        self.begin_resolution_trace(generation);
        let panicked = catch_unwind(AssertUnwindSafe(|| {
            self.resolve_track(track, generation, cancellation)
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
    ) {
        let deadline = ResolutionDeadline::new(cancellation);
        let mut candidates = Vec::new();
        let cached = self.inner.cache.load(&track.key).and_then(|cached| {
            let valid = cached.snapshot.status != LyricsStatus::Ready
                || is_plausible_timeline(&track, &cached.snapshot.lines);
            if valid {
                Some(cached)
            } else {
                if let Err(error) = self.inner.cache.remove(&track.key) {
                    log::warn!("清理时间轴异常的歌词缓存失败: {error}");
                }
                None
            }
        });
        self.record_resolution_step(
            generation,
            "Muse Tune 缓存",
            if cached.is_some() {
                LyricsResolutionOutcome::Hit
            } else {
                LyricsResolutionOutcome::Miss
            },
            cached.as_ref().map(|cached| {
                if cached.is_fresh {
                    "有效期内".to_owned()
                } else {
                    "已过期，作为兜底候选".to_owned()
                }
            }),
        );
        if let Some(cached) = cached {
            if cached.is_fresh {
                let should_check_local = cached.snapshot.status == LyricsStatus::Unavailable
                    || cached.snapshot.precision != Some(super::model::LyricsPrecision::Word)
                    || cached
                        .snapshot
                        .source
                        .as_ref()
                        .is_some_and(|source| source.kind != LyricsSourceKind::Local);
                if should_check_local {
                    match players::resolve_current_local(&track, self.cache_path(track.player)) {
                        Ok(Some(local))
                            if is_plausible_timeline(&track, &local.lines)
                                && (has_word_timing(&local.lines)
                                    || cached.snapshot.status == LyricsStatus::Unavailable) =>
                        {
                            self.record_resolution_step(
                                generation,
                                "当前播放器本地升级",
                                LyricsResolutionOutcome::Hit,
                                Some("发现更高精度或新生成的本地歌词".to_owned()),
                            );
                            self.publish_resolution(&track, local, generation);
                            return;
                        }
                        Ok(_) => {}
                        Err(error) => log::debug!("检查播放器本地歌词升级失败: {error}"),
                    }
                }
                self.publish_if_current_with_method(
                    cached.snapshot,
                    generation,
                    LyricsResolutionMethod::ApplicationCache,
                );
                return;
            }
            if let Some(source) = cached.snapshot.source {
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
            Ok(Some(resolved)) if is_plausible_timeline(&track, &resolved.lines) => {
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
            Ok(Some(_)) => {
                self.record_resolution_step(
                    generation,
                    "当前播放器本地",
                    LyricsResolutionOutcome::Error,
                    Some("歌词时间轴超出歌曲有效范围".to_owned()),
                );
                log::warn!("当前播放器本地歌词时间轴超出歌曲有效范围");
            }
            Ok(None) => self.record_resolution_step(
                generation,
                "当前播放器本地",
                LyricsResolutionOutcome::Miss,
                None,
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
            match players::resolve_qq_local(&track, qq_cache_path) {
                Ok(Some(resolved)) if is_plausible_timeline(&track, &resolved.lines) => {
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
                Ok(Some(_)) => self.record_resolution_step(
                    generation,
                    "QQ 本地兜底",
                    LyricsResolutionOutcome::Error,
                    Some("歌词时间轴超出歌曲有效范围".to_owned()),
                ),
                Ok(None) => self.record_resolution_step(
                    generation,
                    "QQ 本地兜底",
                    LyricsResolutionOutcome::Miss,
                    None,
                ),
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

        if !self.inner.allow_online.load(Ordering::Acquire) {
            if let Some(candidate) = select_best_candidate(&track, candidates) {
                self.publish_candidate(&track, candidate, generation);
            } else {
                self.store_and_publish_if_current(
                    LyricsSnapshot::unavailable(Some(track.key), "联网策略仅允许本地与缓存"),
                    generation,
                    LyricsResolutionMethod::None,
                );
            }
            return;
        }

        let (current_online_result, qq_online_result, netease_result) = thread::scope(|scope| {
            let current_online = players::supports_current_online(track.player).then(|| {
                scope.spawn(|| {
                    players::resolve_current_online(&track, path, &self.inner.client, &deadline)
                })
            });
            let qq_online = (track.player != MediaPlayer::QqMusic).then(|| {
                scope.spawn(|| players::resolve_qq_online(&track, &self.inner.client, &deadline))
            });
            let netease = (track.player != MediaPlayer::NeteaseCloudMusic).then(|| {
                scope.spawn(|| {
                    players::resolve_netease_online(&track, &self.inner.client, &deadline)
                })
            });
            (
                current_online.and_then(|handle| handle.join().ok()),
                qq_online.and_then(|handle| handle.join().ok()),
                netease.and_then(|handle| handle.join().ok()),
            )
        });
        for (label, result) in [
            ("当前播放器在线", current_online_result.as_ref()),
            ("QQ 在线兜底", qq_online_result.as_ref()),
            ("网易云在线兜底", netease_result.as_ref()),
        ] {
            if let Some(result) = result {
                let (outcome, detail) = summarize_resolution_result(result);
                self.record_resolution_step(generation, label, outcome, detail);
            }
        }
        candidates.extend(
            [current_online_result, qq_online_result, netease_result]
                .into_iter()
                .flatten()
                .filter_map(|result| match result {
                    Ok(value) => value.map(candidate_from_source),
                    Err(LyricsError::Cancelled) => None,
                    Err(error) => {
                        log::warn!("跨平台歌词适配器失败: {error}");
                        None
                    }
                }),
        );
        if let Some(candidate) = select_best_candidate(&track, candidates) {
            self.publish_candidate(&track, candidate, generation);
        } else {
            self.store_and_publish_if_current(
                LyricsSnapshot::unavailable(Some(track.key), "没有找到可靠歌词"),
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
        let snapshot = LyricsSnapshot::ready(track.key.clone(), resolved);
        self.store_and_publish_if_current(snapshot, generation, resolution_method);
    }

    /// 发布已带有实际取得方式的候选，区分在线来源与应用缓存命中。
    fn publish_candidate(
        &self,
        track: &TrackDescriptor,
        candidate: LyricsCandidate,
        generation: u64,
    ) {
        let snapshot = LyricsSnapshot::ready(track.key.clone(), candidate.resolved);
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
        if let Ok(mut state) = self.inner.runtime_state.write() {
            state.snapshot.clone_from(&snapshot);
            state.resolution_method = resolution_method;
        }
        (self.inner.publisher)(&snapshot);
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

    fn record_resolution_step(
        &self,
        generation: u64,
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
            });
        }
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
        let next_paths = players::SUPPORTED_PLAYERS
            .into_iter()
            .map(|player| (player, players::automatic_cache_path(player)))
            .collect::<HashMap<_, _>>();
        if let Ok(mut current) = self.inner.adapter_paths.write() {
            current.clone_from(&next_paths);
        }
        let mut next_watchers = HashMap::new();
        for player in players::SUPPORTED_PLAYERS {
            let cache_path = next_paths.get(&player).cloned().flatten();
            let paths = players::watch_paths_for(player, cache_path.as_deref());
            let weak_inner = Arc::downgrade(&self.inner);
            let callback: Arc<dyn Fn(Vec<PathBuf>) + Send + Sync> = Arc::new(move |paths| {
                let Some(inner) = weak_inner.upgrade() else {
                    return;
                };
                let service = LyricsService { inner };
                let configuration_changed = paths.iter().any(|path| {
                    path.file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(|value| value.eq_ignore_ascii_case("KuGou.ini"))
                });
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

    /// 启动 QQ 音乐注册表键值通知；回调只持有服务的弱引用。
    fn start_registry_watcher(&self) {
        let weak_inner = Arc::downgrade(&self.inner);
        players::watch_registry_settings(Arc::new(move || {
            let Some(inner) = weak_inner.upgrade() else {
                return;
            };
            let service = LyricsService { inner };
            if let Err(error) = service.handle_configuration_change(MediaPlayer::QqMusic) {
                log::warn!("响应 QQ 音乐缓存目录设置变化失败: {error}");
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
                .and_then(|result| result.as_ref().ok().and_then(Option::as_ref))
                .is_some_and(|local| self.current_snapshot_matches(&track.key, local))
        {
            return Ok(());
        }
        // 播放器切歌会改写队列和行级歌词缓存，不能因此淘汰其他来源的逐字结果。
        if current_has_word_timing && !current_uses_player_local {
            let local_can_replace_word_timing = local_after_change
                .as_ref()
                .and_then(|result| result.as_ref().ok().and_then(Option::as_ref))
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

fn candidate_from_source(resolved: ResolvedLyrics) -> LyricsCandidate {
    LyricsCandidate {
        resolution_method: method_from_source(&resolved),
        resolved,
    }
}

fn method_from_source(resolved: &ResolvedLyrics) -> LyricsResolutionMethod {
    match resolved.source.kind {
        LyricsSourceKind::Local => LyricsResolutionMethod::PlayerLocal,
        LyricsSourceKind::Online => LyricsResolutionMethod::Online,
    }
}

fn summarize_resolution_result(
    result: &Result<Option<ResolvedLyrics>, LyricsError>,
) -> (LyricsResolutionOutcome, Option<String>) {
    match result {
        Ok(Some(resolved)) => (LyricsResolutionOutcome::Hit, Some(source_summary(resolved))),
        Ok(None) => (LyricsResolutionOutcome::Miss, None),
        Err(error) => (LyricsResolutionOutcome::Error, Some(error.to_string())),
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

/// 拒绝明显超出歌曲时长或顺序倒退的解析结果，避免错误候选进入长期缓存。
fn is_plausible_timeline(track: &TrackDescriptor, lines: &[super::model::LyricLine]) -> bool {
    let Some(duration_ms) = track.duration_ms else {
        return false;
    };
    if lines.is_empty()
        || lines
            .windows(2)
            .any(|pair| pair[0].start_ms > pair[1].start_ms)
    {
        return false;
    }
    let allowed_end = duration_ms.saturating_add(10_000);
    lines.iter().all(|line| {
        line.start_ms <= allowed_end
            && line.words.iter().all(|word| {
                word.end_ms > word.start_ms
                    && word.start_ms >= line.start_ms
                    && word.start_ms <= allowed_end
            })
    })
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

    use super::{TrackDescriptor, is_plausible_timeline};
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
