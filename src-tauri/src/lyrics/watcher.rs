use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// 一轮写入的安静窗口：同一批歌词落盘往往是多次写入，目录被删除重建时还会从父目录产生事件，
/// 这里等最后一次事件之后再安静满这么久，才把整批合并成一次回调。
/// 调小会让同一次写入被拆成多次解析，调大则会让歌词刷新明显滞后于文件实际变化。
const WRITE_SETTLE_TIME: Duration = Duration::from_millis(400);

/// 同时拥有原生目录监听器和事件归并线程，保证释放时先断开生产者再回收消费者。
pub struct LyricsFileWatcher {
    watcher: Option<RecommendedWatcher>,
    signal: Arc<WatcherSignal>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct WatcherState {
    changed_paths: HashSet<PathBuf>,
    last_change: Option<Instant>,
    stopped: bool,
}

#[derive(Default)]
struct WatcherSignal {
    state: Mutex<WatcherState>,
    changed: Condvar,
}

impl Drop for LyricsFileWatcher {
    fn drop(&mut self) {
        // 先释放 notify watcher，确保停止信号之后不会再有生产者写入。
        drop(self.watcher.take());
        {
            let mut state = self
                .signal
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.stopped = true;
            state.changed_paths.clear();
            state.last_change = None;
        }
        self.signal.changed.notify_one();
        if let Some(worker) = self.worker.take() {
            if worker.thread().id() == thread::current().id() {
                // 缓存根目录变化可由本 worker 触发 watcher 重建；当前线程将在 callback 返回后自然退出。
                log::debug!("歌词文件监听 worker 正在重建自身，跳过自 join");
                return;
            }
            if worker.join().is_err() {
                log::warn!("歌词文件监听线程异常退出");
            }
        }
    }
}

/// 建立非递归目录监听，并在一轮连续写入安静后只回调一次。
pub fn create(
    paths: impl IntoIterator<Item = PathBuf>,
    on_change: Arc<dyn Fn(Vec<PathBuf>) + Send + Sync>,
) -> Result<Option<LyricsFileWatcher>, notify::Error> {
    let requested_paths = paths.into_iter().collect::<HashSet<_>>();
    let watch_roots = requested_paths
        .iter()
        .flat_map(|path| watch_roots_for_target(path))
        .collect::<HashSet<_>>();
    if watch_roots.is_empty() {
        return Ok(None);
    }

    let signal = Arc::new(WatcherSignal::default());
    let event_targets = requested_paths.clone();
    let producer_signal = Arc::clone(&signal);
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        if let Ok(event) = result
            && is_content_change(&event.kind)
        {
            let paths = event
                .paths
                .into_iter()
                .filter_map(|path| {
                    if is_lyrics_source_path(&path) {
                        return Some(path);
                    }
                    event_targets
                        .iter()
                        .find(|target| {
                            paths_equivalent(target, &path) || path_is_ancestor(&path, target)
                        })
                        .cloned()
                })
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                {
                    let mut state = producer_signal
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if state.stopped {
                        return;
                    }
                    state.changed_paths.extend(paths);
                    state.last_change = Some(Instant::now());
                }
                producer_signal.changed.notify_one();
            }
        }
    })?;
    for path in watch_roots {
        watcher.watch(&path, RecursiveMode::NonRecursive)?;
    }

    let worker_signal = Arc::clone(&signal);
    let worker = thread::Builder::new()
        .name("lyrics-cache-events".to_owned())
        .spawn(move || {
            while let Some(changed_paths) = wait_for_quiet_batch(&worker_signal) {
                on_change(changed_paths.into_iter().collect());
            }
        })
        .map_err(notify::Error::io)?;
    Ok(Some(LyricsFileWatcher {
        watcher: Some(watcher),
        signal,
        worker: Some(worker),
    }))
}

/// 等待至少一个文件事件，并以最后一批事件为起点保持完整安静窗口。
fn wait_for_quiet_batch(signal: &WatcherSignal) -> Option<HashSet<PathBuf>> {
    let mut state = signal
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    while state.changed_paths.is_empty() && !state.stopped {
        state = signal
            .changed
            .wait(state)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    }
    loop {
        if state.stopped {
            return None;
        }
        let Some(last_change) = state.last_change else {
            continue;
        };
        let remaining = (last_change + WRITE_SETTLE_TIME).saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let paths = std::mem::take(&mut state.changed_paths);
            state.last_change = None;
            return Some(paths);
        }
        let (next_state, _) = signal
            .changed
            .wait_timeout(state, remaining)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state = next_state;
    }
}

/// 比较 Windows 路径时忽略扩展长度前缀、分隔符形式和大小写差异。
pub fn paths_equivalent(left: &Path, right: &Path) -> bool {
    comparable_path(left) == comparable_path(right)
}

fn path_is_ancestor(parent: &Path, child: &Path) -> bool {
    let parent = comparable_path(parent);
    let child = comparable_path(child);
    child == parent || child.starts_with(&format!("{parent}\\"))
}

fn comparable_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', r"\")
        .to_lowercase()
}

/// 目标目录尚未创建时退到最近的现存父目录，以便在创建事件后重建精确监听。
fn nearest_existing_directory(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|candidate| candidate.is_dir())
        .map(Path::to_path_buf)
}

/// 已存在目标同时监听父目录以捕获删除与重建；缺失目标退到最近的现存祖先。
fn watch_roots_for_target(path: &Path) -> Vec<PathBuf> {
    if !path.is_dir() {
        return nearest_existing_directory(path).into_iter().collect();
    }
    let mut roots = vec![path.to_path_buf()];
    if let Some(parent) = path.parent().filter(|parent| parent.is_dir()) {
        roots.push(parent.to_path_buf());
    }
    roots
}

/// 只接受解析器真正读取的歌词、队列缓存和播放器目录配置文件。
fn is_lyrics_source_path(path: &Path) -> bool {
    let file_name = path.file_name().and_then(|value| value.to_str());
    if file_name.is_some_and(|value| {
        value.eq_ignore_ascii_case("QueueCache")
            || value.eq_ignore_ascii_case("KuGou.ini")
            || value.eq_ignore_ascii_case("playingList")
            || is_netease_cache_name(value)
    }) {
        return true;
    }
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "qrc" | "krc" | "lrc" | "yrc" | "json"
            )
        })
}

fn is_netease_cache_name(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// 仅响应可能改变歌词内容或文件集合的事件。
fn is_content_change(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Any | EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}
