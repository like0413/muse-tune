use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use super::metrics::WorkerMessageKind;

/// 允许按最新状态合并的系统通知；需要响应的 command 不进入这里。
pub(in crate::media) enum WorkerEvent {
    Manager,
    MediaProperties {
        session_id: u64,
        observed_at: Instant,
    },
    PlaybackInfo(u64),
    TimelineProperties(u64),
    Volume(u64),
    VolumeSessions(u64),
}

impl WorkerEvent {
    /// 返回与 A4 诊断字段保持兼容的逻辑消息分类。
    pub(super) const fn kind(&self) -> WorkerMessageKind {
        match self {
            Self::Manager => WorkerMessageKind::ManagerChanged,
            Self::MediaProperties { .. } => WorkerMessageKind::MediaPropertiesChanged,
            Self::PlaybackInfo(_) => WorkerMessageKind::PlaybackInfoChanged,
            Self::TimelineProperties(_) => WorkerMessageKind::TimelinePropertiesChanged,
            Self::Volume(_) => WorkerMessageKind::VolumeChanged,
            Self::VolumeSessions(_) => WorkerMessageKind::VolumeSessionsChanged,
        }
    }
}

/// callback 共享的有限 pending 状态；同一会话或音量目标只保留一份工作。
#[derive(Default)]
pub(super) struct PendingWorkerEvents {
    pub(super) wake_enqueued: bool,
    manager_changed: bool,
    media_properties_changed: HashMap<u64, Instant>,
    playback_info_changed: HashSet<u64>,
    timeline_properties_changed: HashSet<u64>,
    volume_changed: HashSet<u64>,
    volume_sessions_changed: HashSet<u64>,
}

impl PendingWorkerEvents {
    /// 合并一条通知，并返回它是否替换了同一逻辑目标的待处理状态。
    pub(super) fn merge(&mut self, event: WorkerEvent) -> bool {
        match event {
            WorkerEvent::Manager => std::mem::replace(&mut self.manager_changed, true),
            WorkerEvent::MediaProperties {
                session_id,
                observed_at,
            } => match self.media_properties_changed.entry(session_id) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    if observed_at > *entry.get() {
                        entry.insert(observed_at);
                    }
                    true
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(observed_at);
                    false
                }
            },
            WorkerEvent::PlaybackInfo(session_id) => !self.playback_info_changed.insert(session_id),
            WorkerEvent::TimelineProperties(session_id) => {
                !self.timeline_properties_changed.insert(session_id)
            }
            WorkerEvent::Volume(target_id) => !self.volume_changed.insert(target_id),
            WorkerEvent::VolumeSessions(target_id) => {
                !self.volume_sessions_changed.insert(target_id)
            }
        }
    }

    /// 原子取走当前批次并允许后续 callback 投递下一次 wake。
    pub(super) fn take_batch(&mut self) -> WorkerEventBatch {
        self.wake_enqueued = false;
        WorkerEventBatch {
            manager_changed: std::mem::take(&mut self.manager_changed),
            media_properties_changed: std::mem::take(&mut self.media_properties_changed),
            playback_info_changed: std::mem::take(&mut self.playback_info_changed),
            timeline_properties_changed: std::mem::take(&mut self.timeline_properties_changed),
            volume_changed: std::mem::take(&mut self.volume_changed),
            volume_sessions_changed: std::mem::take(&mut self.volume_sessions_changed),
        }
    }
}

/// worker 单次 wake 对应的逻辑事件集合。
pub(super) struct WorkerEventBatch {
    pub(super) manager_changed: bool,
    pub(super) media_properties_changed: HashMap<u64, Instant>,
    pub(super) playback_info_changed: HashSet<u64>,
    pub(super) timeline_properties_changed: HashSet<u64>,
    pub(super) volume_changed: HashSet<u64>,
    pub(super) volume_sessions_changed: HashSet<u64>,
}
