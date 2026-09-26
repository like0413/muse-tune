//! 歌词服务的门面：持有共享状态、生命周期与对 Tauri command 暴露的入口。
//!
//! 具体职责分布在同级的子模块里：
//! - `scheduler`：解析任务的排队与串行消费；
//! - `resolution`：一轮解析的执行与结论判定；
//! - `publisher`：快照的读取、代际校验与发布；
//! - `diagnostics`：设置页与数据页的只读视图；
//! - `watch_coordinator`：播放器缓存目录的文件与注册表监听；
//! - `coordinator` / `preferences` / `cache_control`：歌曲协调、偏好与缓存控制；
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

use crate::{media::MediaPlayer, storage::StoragePaths};

use super::settings::LyricsPreferences;
use super::{
    cache::{CacheLookup, ParsedLyricsCache},
    error::LyricsError,
    model::{
        LyricsLookupOutcome, LyricsResolutionMethod, LyricsResolutionRecord, LyricsResolutionStep,
        LyricsResolutionTrack, LyricsSnapshot,
    },
    players,
    track::TrackDescriptor,
    watcher::LyricsFileWatcher,
};
use players::RegistryWatchHandle;

mod cache_control;
mod cache_policy;
mod coordinator;
mod diagnostics;
mod executor;
mod pipeline;
mod plan;
mod preferences;
mod publisher;
mod resolution;
mod scheduler;
mod trace;
mod watch_coordinator;

const LYRICS_CHANGED_EVENT: &str = "lyrics://changed";
const LYRICS_DIAGNOSTICS_CHANGED_EVENT: &str = "lyrics://diagnostics-changed";
/// 单次 HTTP 请求的总超时，覆盖连接、重定向与读取响应体的全过程。
/// 超时按“该来源失败”处理并交给解析层判断结论（见 `publish_no_lyrics`），这里不做重试。
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
    registry_watchers: Mutex<Vec<RegistryWatchHandle>>,
    shutdown_requested: AtomicBool,
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
    cached: Option<CacheLookup>,
    /// 起轮时没有缓存是否因为本次流程主动清除了它；只影响诊断文案。
    cache_cleared: bool,
}

#[derive(Default)]
struct LyricsRuntimeState {
    /// 当前已按设置转换的歌词；与磁盘缓存保持相同字形，设置切换时从这里即时重绘。
    source_snapshot: LyricsSnapshot,
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
        let storage = app.state::<StoragePaths>();
        let cache = ParsedLyricsCache::new(storage.cache_directory())?;
        let client = Client::builder()
            .timeout(NETWORK_TIMEOUT)
            // 连接阶段单独设更短的上限：握不上手时尽快换下一个来源，而已建立的连接仍可用满总超时。
            .connect_timeout(Duration::from_secs(4))
            // 重定向逐跳校验且最多 3 跳：`Location` 由第三方平台返回、不在我们的控制范围内，
            // 不校验就会让一次本来合法的请求被 302 到任意主机（内网探测、明文降级）；
            // 跳数不设上限则等于允许一条可以无限接力的跳转链。两个条件缺一不可。
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
        let preferences = super::settings::restore_lyrics_preferences(app);
        let service = Self {
            inner: Arc::new(LyricsServiceInner {
                cache,
                client,
                current_track: Mutex::new(None),
                generation: AtomicU64::new(0),
                preferences: RwLock::new(preferences),
                publisher,
                diagnostics_notifier,
                runtime_state: RwLock::new(LyricsRuntimeState::default()),
                adapter_paths: RwLock::new(HashMap::new()),
                watchers: Mutex::new(HashMap::new()),
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
}

/// 放行条件：必须同时是 HTTPS、且主机在 [`ALLOWED_HTTPS_HOSTS`] 内。
///
/// 这两条一起构成出站白名单：协议只允许加密传输（不接受明文 HTTP），目标只允许已知的歌词平台，
/// 避免播放器上报的元数据或平台返回的重定向把我们带到任意主机。
fn is_allowed_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| ALLOWED_HTTPS_HOSTS.contains(&host))
}
