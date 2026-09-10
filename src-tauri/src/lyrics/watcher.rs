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
    let paths = paths
        .into_iter()
        .filter(|path| path.is_dir())
        .collect::<HashSet<_>>();
    if paths.is_empty() {
        return Ok(None);
    }

    let (sender, receiver) = mpsc::channel::<Vec<PathBuf>>();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        if let Ok(event) = result
            && is_content_change(&event.kind)
        {
            let paths = event
                .paths
                .into_iter()
                .filter(|path| is_lyrics_source_path(path))
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                let _ = sender.send(paths);
            }
        }
    })?;
    for path in paths {
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
