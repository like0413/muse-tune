use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

const WRITE_SETTLE_TIME: Duration = Duration::from_millis(400);

/// 建立非递归目录监听，并在一轮连续写入安静后只回调一次。
pub fn create(
    paths: impl IntoIterator<Item = PathBuf>,
    on_change: Arc<dyn Fn(Vec<PathBuf>) + Send + Sync>,
) -> Result<Option<RecommendedWatcher>, notify::Error> {
    let requested_paths = paths.into_iter().collect::<HashSet<_>>();
    let watch_roots = requested_paths
        .iter()
        .flat_map(|path| watch_roots_for_target(path))
        .collect::<HashSet<_>>();
    if watch_roots.is_empty() {
        return Ok(None);
    }

    let (sender, receiver) = mpsc::channel::<Vec<PathBuf>>();
    let event_targets = requested_paths.clone();
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
                let _ = sender.send(paths);
            }
        }
    })?;
    for path in watch_roots {
        watcher.watch(&path, RecursiveMode::NonRecursive)?;
    }

    thread::Builder::new()
        .name("lyrics-cache-events".to_owned())
        .spawn(move || {
            while let Ok(paths) = receiver.recv() {
                let mut changed_paths = paths.into_iter().collect::<HashSet<_>>();
                while let Ok(paths) = receiver.recv_timeout(WRITE_SETTLE_TIME) {
                    changed_paths.extend(paths);
                }
                on_change(changed_paths.into_iter().collect());
            }
        })
        .map_err(notify::Error::io)?;
    Ok(Some(watcher))
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{is_lyrics_source_path, paths_equivalent};

    #[test]
    fn source_filter_accepts_current_player_files() {
        for path in [
            "QueueCache",
            "KuGou.ini",
            "playingList",
            "0123456789abcdef0123456789abcdef",
            "song_qm.qrc",
            "song.krc",
        ] {
            assert!(is_lyrics_source_path(Path::new(path)), "未识别 {path}");
        }
    }

    #[test]
    fn source_filter_rejects_unrelated_cache_files() {
        assert!(!is_lyrics_source_path(Path::new("cover.jpg")));
    }

    #[test]
    fn path_comparison_ignores_windows_extended_prefix() {
        assert!(paths_equivalent(
            Path::new(r"C:\Music\Lyrics"),
            Path::new(r"\\?\c:\music\lyrics")
        ));
    }
}
