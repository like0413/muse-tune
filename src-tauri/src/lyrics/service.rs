use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use notify::RecommendedWatcher;
use reqwest::{Url, blocking::Client, redirect};
use tauri::{Emitter, Manager, Runtime};

use crate::media::{MediaPlayer, MediaSessionSnapshot};

use super::{
    cache::ParsedLyricsCache,
    model::{LyricsCachePathState, LyricsSnapshot, LyricsStatus, ResolvedLyrics},
    players,
    settings::LyricsPathSettings,
    track::TrackDescriptor,
    watcher,
};

const LYRICS_CHANGED_EVENT: &str = "lyrics://changed";
const CACHE_PATHS_CHANGED_EVENT: &str = "lyrics://cache-paths-changed";
const NETWORK_TIMEOUT: Duration = Duration::from_secs(8);
const ALLOWED_HTTPS_HOSTS: [&str; 3] = ["c.y.qq.com", "music.163.com", "beta-luna.douyin.com"];

type SnapshotPublisher = dyn Fn(&LyricsSnapshot) + Send + Sync;

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
    paths: LyricsPathSettings,
    publisher: Arc<SnapshotPublisher>,
    path_publisher: Arc<dyn Fn(Vec<LyricsCachePathState>) + Send + Sync>,
    snapshot: RwLock<LyricsSnapshot>,
    watcher: Mutex<Option<RecommendedWatcher>>,
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
        let publisher = Arc::new(move |snapshot: &LyricsSnapshot| {
            if let Err(error) = app_handle.emit(LYRICS_CHANGED_EVENT, snapshot) {
                log::warn!("广播歌词状态失败: {error}");
            }
        });
        let app_handle = app.handle().clone();
        let path_publisher = Arc::new(move |paths: Vec<LyricsCachePathState>| {
            if let Err(error) = app_handle.emit(CACHE_PATHS_CHANGED_EVENT, paths) {
                log::warn!("广播歌词目录状态失败: {error}");
            }
        });
        let service = Self {
            inner: Arc::new(LyricsServiceInner {
                cache,
                client,
                current_track: Mutex::new(None),
                generation: AtomicU64::new(0),
                enabled: AtomicBool::new(super::settings::restore_lyrics_enabled(app)),
                paths: LyricsPathSettings::restore(app),
                publisher,
                path_publisher,
                snapshot: RwLock::new(LyricsSnapshot::default()),
                watcher: Mutex::new(None),
            }),
        };
        service.refresh_watcher();
        service.start_registry_watcher();
        Ok(service)
    }

    /// 返回最近一次歌词快照。
    pub fn snapshot(&self) -> LyricsSnapshot {
        self.inner
            .snapshot
            .read()
            .map_or_else(|_| LyricsSnapshot::default(), |snapshot| snapshot.clone())
    }

    /// 返回设置页所需的目录状态。
    pub fn cache_paths(&self) -> Vec<LyricsCachePathState> {
        self.inner.paths.states()
    }

    /// 更新歌词总开关；关闭时取消解析，开启时立即解析当前歌曲。
    pub fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        if self.inner.enabled.swap(enabled, Ordering::AcqRel) == enabled {
            return Ok(());
        }
        if enabled {
            self.force_resolve_current()
        } else {
            self.inner.generation.fetch_add(1, Ordering::AcqRel);
            let track_key = self
                .inner
                .current_track
                .lock()
                .map_err(|_| "当前歌曲状态不可用".to_owned())?
                .as_ref()
                .map(|track| track.key.clone());
            self.publish(LyricsSnapshot::unavailable(track_key, "歌词显示已关闭"));
            Ok(())
        }
    }

    /// 更新目录覆盖并强制当前歌曲重新解析。
    pub fn set_cache_path_override(
        &self,
        player: MediaPlayer,
        path: Option<PathBuf>,
    ) -> Result<(), String> {
        self.inner.paths.set_override(player, path)?;
        self.refresh_watcher();
        if let Err(error) = self.inner.cache.clear() {
            log::warn!("清理旧歌词解析缓存失败: {error}");
        }
        self.force_resolve_current()?;
        Ok(())
    }

    /// 接收媒体模块的完整快照变化，时间线轻量事件不会触发此入口。
    pub(crate) fn update_media(&self, snapshot: Option<&MediaSessionSnapshot>) {
        self.update_track(snapshot.and_then(TrackDescriptor::from_snapshot));
    }

    fn update_track(&self, track: Option<TrackDescriptor>) {
        let changed = self
            .inner
            .current_track
            .lock()
            .map(|mut current| {
                if *current == track {
                    false
                } else {
                    current.clone_from(&track);
                    true
                }
            })
            .unwrap_or(false);
        if !changed {
            return;
        }
        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let Some(track) = track else {
            self.publish(LyricsSnapshot::unavailable(None, "当前没有支持的歌曲"));
            return;
        };
        if !self.inner.enabled.load(Ordering::Acquire) {
            self.publish(LyricsSnapshot::unavailable(
                Some(track.key),
                "歌词显示已关闭",
            ));
            return;
        }
        // 最新版酷狗没有有效 GSMTC 时间线时无法可靠同步歌词，必须在读缓存和启动解析前回退。
        if track.player == MediaPlayer::KugouMusic && track.duration_ms.is_none() {
            self.publish(LyricsSnapshot::unavailable(
                Some(track.key),
                "酷狗音乐未提供有效播放时间线",
            ));
            return;
        }
        self.publish(LyricsSnapshot::loading(track.key.clone()));
        let service = self.clone();
        if let Err(error) = thread::Builder::new()
            .name("lyrics-resolver".to_owned())
            .spawn(move || service.resolve_track_guarded(track, generation))
        {
            self.publish(LyricsSnapshot {
                status: LyricsStatus::Error,
                error_reason: Some(format!("启动歌词解析线程失败: {error}")),
                ..LyricsSnapshot::default()
            });
        }
    }

    /// 隔离单个平台或第三方库的非预期 panic，确保状态不会永久停留在加载中。
    fn resolve_track_guarded(&self, track: TrackDescriptor, generation: u64) {
        let track_key = track.key.clone();
        if catch_unwind(AssertUnwindSafe(|| self.resolve_track(track, generation))).is_err() {
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

    fn resolve_track(&self, track: TrackDescriptor, generation: u64) {
        if let Some(cached) = self.inner.cache.load(&track.key) {
            self.publish_if_current(cached, generation);
            return;
        }
        let path = self.inner.paths.effective_path(track.player);
        let current_result = players::resolve_current_player(&track, path, &self.inner.client);
        let current_failed = current_result.is_err();
        match current_result {
            Ok(Some(resolved)) => {
                self.finish_resolution(&track, resolved, generation);
                return;
            }
            Ok(None) => {}
            Err(error) => log::warn!("当前播放器歌词适配器失败: {error}"),
        }
        if !self.is_current_generation(generation) {
            return;
        }

        let (qq_result, netease_result) = thread::scope(|scope| {
            let qq = (track.player != MediaPlayer::QqMusic || current_failed)
                .then(|| scope.spawn(|| players::resolve_qq_online(&track, &self.inner.client)));
            let netease =
                (track.player != MediaPlayer::NeteaseCloudMusic || current_failed).then(|| {
                    scope.spawn(|| players::resolve_netease_online(&track, &self.inner.client))
                });
            (
                qq.and_then(|handle| handle.join().ok()),
                netease.and_then(|handle| handle.join().ok()),
            )
        });
        let resolved = [qq_result, netease_result]
            .into_iter()
            .flatten()
            .filter_map(|result| match result {
                Ok(value) => value,
                Err(error) => {
                    log::warn!("跨平台歌词适配器失败: {error}");
                    None
                }
            })
            .next();
        if let Some(resolved) = resolved {
            self.finish_resolution(&track, resolved, generation);
        } else {
            self.publish_if_current(
                LyricsSnapshot::unavailable(Some(track.key), "没有找到可靠歌词"),
                generation,
            );
        }
    }

    fn finish_resolution(
        &self,
        track: &TrackDescriptor,
        resolved: ResolvedLyrics,
        generation: u64,
    ) {
        let snapshot = LyricsSnapshot::ready(track.key.clone(), resolved);
        if !self.is_current_generation(generation) {
            return;
        }
        if let Err(error) = self.inner.cache.store(&snapshot) {
            log::warn!("保存解析后歌词缓存失败: {error}");
        }
        self.publish_if_current(snapshot, generation);
    }

    fn publish_if_current(&self, snapshot: LyricsSnapshot, generation: u64) {
        if self.is_current_generation(generation) {
            self.publish(snapshot);
        }
    }

    fn is_current_generation(&self, generation: u64) -> bool {
        self.inner.generation.load(Ordering::Acquire) == generation
    }

    fn publish(&self, snapshot: LyricsSnapshot) {
        if let Ok(mut current) = self.inner.snapshot.write() {
            current.clone_from(&snapshot);
        }
        (self.inner.publisher)(&snapshot);
    }

    /// 重建当前有效目录的非递归监听器；无目录时不启动后台线程。
    fn refresh_watcher(&self) {
        let states = self.inner.paths.states();
        let paths = states
            .iter()
            .filter_map(|state| state.effective_path.as_ref().map(PathBuf::from))
            .chain(players::configuration_watch_paths());
        let weak_inner = Arc::downgrade(&self.inner);
        let callback: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let Some(inner) = weak_inner.upgrade() else {
                return;
            };
            let service = LyricsService { inner };
            if let Err(error) = service.handle_cache_change() {
                log::warn!("响应播放器歌词缓存变化失败: {error}");
            }
        });
        match watcher::create(paths, callback) {
            Ok(next) => {
                if let Ok(mut current) = self.inner.watcher.lock() {
                    *current = next;
                }
            }
            Err(error) => log::warn!("建立播放器歌词缓存监听失败: {error}"),
        }
        (self.inner.path_publisher)(states);
    }

    /// 启动 QQ 音乐注册表键值通知；回调只持有服务的弱引用。
    fn start_registry_watcher(&self) {
        let weak_inner = Arc::downgrade(&self.inner);
        players::watch_registry_settings(Arc::new(move || {
            let Some(inner) = weak_inner.upgrade() else {
                return;
            };
            let service = LyricsService { inner };
            if let Err(error) = service.handle_cache_change() {
                log::warn!("响应 QQ 音乐缓存目录设置变化失败: {error}");
            }
        }));
    }

    /// 缓存文件变化后清理规范化缓存并重新解析当前歌曲。
    fn handle_cache_change(&self) -> Result<(), String> {
        self.refresh_watcher();
        if let Err(error) = self.inner.cache.clear() {
            log::warn!("清理发生变化的歌词缓存失败: {error}");
        }
        self.force_resolve_current()
    }

    /// 保留当前歌曲身份，但提升代数并重新执行一次解析。
    fn force_resolve_current(&self) -> Result<(), String> {
        let current = self
            .inner
            .current_track
            .lock()
            .map_err(|_| "当前歌曲状态不可用".to_owned())?
            .take();
        self.update_track(current);
        Ok(())
    }
}

fn is_allowed_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| ALLOWED_HTTPS_HOSTS.contains(&host))
}
