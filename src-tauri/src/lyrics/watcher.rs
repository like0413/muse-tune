use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

const WRITE_SETTLE_TIME: Duration = Duration::from_millis(400);

/// 建立非递归目录监听，并在一轮连续写入安静后只回调一次。
pub fn create(
    paths: impl IntoIterator<Item = PathBuf>,
    on_change: Arc<dyn Fn() + Send + Sync>,
) -> Result<Option<RecommendedWatcher>, notify::Error> {
    let paths = paths
        .into_iter()
        .filter(|path| path.is_dir())
        .collect::<HashSet<_>>();
    if paths.is_empty() {
        return Ok(None);
    }

    let (sender, receiver) = mpsc::sync_channel::<()>(1);
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        if result
            .as_ref()
            .is_ok_and(|event| is_content_change(&event.kind))
        {
            let _ = sender.try_send(());
        }
    })?;
    for path in paths {
        watcher.watch(&path, RecursiveMode::NonRecursive)?;
    }

    thread::Builder::new()
        .name("lyrics-cache-events".to_owned())
        .spawn(move || {
            while receiver.recv().is_ok() {
                while receiver.recv_timeout(WRITE_SETTLE_TIME).is_ok() {}
                on_change();
            }
        })
        .map_err(notify::Error::io)?;
    Ok(Some(watcher))
}

/// 仅响应可能改变歌词内容或文件集合的事件。
fn is_content_change(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Any | EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}
