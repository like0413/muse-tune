//! 歌词服务的门面：持有共享状态、生命周期与对 Tauri command 暴露的入口。
//!
//! 具体职责分布在同级的子模块里：
//! - `scheduler`：解析任务的排队与串行消费；
//! - `resolution`：一轮解析的执行与结论判定；
//! - `publisher`：快照的读取、代际校验与发布；
//! - `diagnostics`：设置页与数据页的只读视图；
//! - `watch_coordinator`：播放器缓存目录的文件与注册表监听；
//! - `plan` / `pipeline` / `executor` / `cache_policy` / `trace`：解析策略与过程记录。

use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
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
        LyricsLookupOutcome, LyricsOnlineStrategy, LyricsResolutionMethod, LyricsResolutionRecord,
        LyricsResolutionStep, LyricsResolutionTrack, LyricsSnapshot, LyricsStatus,
    },
    players,
    track::TrackDescriptor,
    watcher::{LyricsFileWatcher, LyricsWatcherMetrics},
};
use pipeline::is_cached_snapshot_displayable;
use players::RegistryWatchHandle;

mod cache_policy;
mod diagnostics;
mod executor;
mod pipeline;
mod plan;
mod publisher;
mod resolution;
mod scheduler;
mod trace;
mod watch_coordinator;

const LYRICS_CHANGED_EVENT: &str = "lyrics://changed";
const LYRICS_DIAGNOSTICS_CHANGED_EVENT: &str = "lyrics://diagnostics-changed";
const NETWORK_TIMEOUT: Duration = Duration::from_secs(8);
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
