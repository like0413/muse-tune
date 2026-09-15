use std::sync::RwLock;

use tauri::{AppHandle, Emitter, Runtime};

use super::{MediaSessionSnapshot, MediaSnapshotSubscriber, MediaVolumeSnapshot, SessionEntry};

const MEDIA_SESSION_CHANGED_EVENT: &str = "media://session-changed";
const MEDIA_TIMELINE_CHANGED_EVENT: &str = "media://timeline-changed";
const MEDIA_VOLUME_CHANGED_EVENT: &str = "media://volume-changed";

/// 聚合完整快照发布所需的只读依赖，不拥有媒体 worker 状态。
pub(super) struct MediaSnapshotPublisher<'a, R: Runtime> {
    pub(super) app: &'a AppHandle<R>,
    pub(super) snapshot: &'a RwLock<Option<MediaSessionSnapshot>>,
    pub(super) subscriber: &'a MediaSnapshotSubscriber,
}

/// 广播应用音量；悬浮窗未显示时事件只更新轻量前端状态。
pub(super) fn publish_volume<R: Runtime>(app: &AppHandle<R>, volume: Option<MediaVolumeSnapshot>) {
    if let Err(error) = app.emit(MEDIA_VOLUME_CHANGED_EVENT, volume) {
        log::warn!("向任务栏广播播放器应用音量失败: {error}");
    }
}

/// 发布已选会话快照；不存在有效目标时清空任务栏媒体状态。
pub(super) fn publish_selected_snapshot<R: Runtime>(
    publisher: &MediaSnapshotPublisher<'_, R>,
    entries: &[SessionEntry],
    selected_id: Option<u64>,
) {
    let next = entries
        .iter()
        .find(|entry| Some(entry.id) == selected_id)
        .map(|entry| entry.snapshot.clone());
    publish_snapshot(publisher, next);
}

/// 仅发布轻量时间线，避免播放器定期更新时间时重复序列化封面。
pub(super) fn publish_selected_timeline<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &RwLock<Option<MediaSessionSnapshot>>,
    entries: &[SessionEntry],
    selected_id: Option<u64>,
) {
    let next = entries
        .iter()
        .find(|entry| Some(entry.id) == selected_id)
        .and_then(|entry| entry.snapshot.timeline.clone());
    if let Ok(mut current) = snapshot.write()
        && let Some(current) = current.as_mut()
    {
        current.timeline.clone_from(&next);
    }
    if let Err(error) = app.emit(MEDIA_TIMELINE_CHANGED_EVENT, &next) {
        log::warn!("向任务栏广播媒体时间线失败: {error}");
    }
}

/// 原子替换缓存并把相同值广播给所有任务栏窗口。
fn publish_snapshot<R: Runtime>(
    publisher: &MediaSnapshotPublisher<'_, R>,
    next: Option<MediaSessionSnapshot>,
) {
    if let Ok(mut current) = publisher.snapshot.write() {
        current.clone_from(&next);
    }
    emit_snapshot(publisher.app, &next);
    (publisher.subscriber)(&next);
}

/// 广播媒体快照；窗口未就绪时由前端初始 command 补取缓存。
fn emit_snapshot<R: Runtime>(app: &AppHandle<R>, snapshot: &Option<MediaSessionSnapshot>) {
    if let Err(error) = app.emit(MEDIA_SESSION_CHANGED_EVENT, snapshot) {
        log::warn!("向任务栏广播媒体会话失败: {error}");
    }
}
