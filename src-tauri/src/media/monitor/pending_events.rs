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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// 首次投递不算合并，重复投递同一逻辑目标才算——调用方靠返回值决定是否需要再次唤醒 worker。
    #[test]
    fn duplicate_events_report_coalescing() {
        let mut pending = PendingWorkerEvents::default();

        assert!(!pending.merge(WorkerEvent::PlaybackInfo(1)));
        assert!(pending.merge(WorkerEvent::PlaybackInfo(1)));
        assert!(!pending.merge(WorkerEvent::PlaybackInfo(2)));
        assert!(!pending.merge(WorkerEvent::Manager));
        assert!(pending.merge(WorkerEvent::Manager));
    }

    /// 媒体属性事件必须保留最新观测时间：旧时间戳覆盖新时间戳会让元数据稳定刷新提前触发。
    #[test]
    fn media_properties_keep_latest_observation() {
        let mut pending = PendingWorkerEvents::default();
        let base = Instant::now();
        let latest = base + Duration::from_millis(200);

        assert!(!pending.merge(WorkerEvent::MediaProperties {
            session_id: 1,
            observed_at: latest,
        }));
        assert!(pending.merge(WorkerEvent::MediaProperties {
            session_id: 1,
            observed_at: base + Duration::from_millis(50),
        }));

        let batch = pending.take_batch();
        assert_eq!(batch.media_properties_changed.get(&1), Some(&latest));
    }

    /// 取走批次后必须清空，否则同一事件会被反复处理。
    #[test]
    fn taking_a_batch_drains_every_channel() {
        let mut pending = PendingWorkerEvents::default();
        let now = Instant::now();
        pending.merge(WorkerEvent::Manager);
        pending.merge(WorkerEvent::MediaProperties {
            session_id: 9,
            observed_at: now,
        });
        pending.merge(WorkerEvent::PlaybackInfo(1));
        pending.merge(WorkerEvent::TimelineProperties(1));
        pending.merge(WorkerEvent::Volume(3));
        pending.merge(WorkerEvent::VolumeSessions(3));
        pending.wake_enqueued = true;

        let batch = pending.take_batch();
        assert!(batch.manager_changed);
        assert_eq!(batch.media_properties_changed.len(), 1);
        assert_eq!(batch.playback_info_changed.len(), 1);
        assert_eq!(batch.timeline_properties_changed.len(), 1);
        assert_eq!(batch.volume_changed.len(), 1);
        assert_eq!(batch.volume_sessions_changed.len(), 1);
        assert!(!pending.wake_enqueued);

        let drained = pending.take_batch();
        assert!(!drained.manager_changed);
        assert!(drained.media_properties_changed.is_empty());
        assert!(drained.playback_info_changed.is_empty());
        assert!(drained.timeline_properties_changed.is_empty());
        assert!(drained.volume_changed.is_empty());
        assert!(drained.volume_sessions_changed.is_empty());
    }

    /// 不同会话的同类事件互不合并，否则多播放器并存时会漏掉其中一个的变化。
    #[test]
    fn sessions_are_tracked_independently() {
        let mut pending = PendingWorkerEvents::default();
        let now = Instant::now();

        assert!(!pending.merge(WorkerEvent::MediaProperties {
            session_id: 1,
            observed_at: now,
        }));
        assert!(!pending.merge(WorkerEvent::MediaProperties {
            session_id: 2,
            observed_at: now,
        }));

        assert_eq!(pending.take_batch().media_properties_changed.len(), 2);
    }
}
