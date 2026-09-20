use std::{
    collections::{HashMap, hash_map::Entry},
    time::{Duration, Instant},
};

const METADATA_SETTLE_DELAY: Duration = Duration::from_millis(300);
const INITIAL_VOLUME_REBIND_DELAYS: [Duration; 4] = [
    Duration::from_millis(300),
    Duration::from_millis(800),
    Duration::from_millis(1_600),
    Duration::from_millis(3_200),
];

/// media worker 到期后需要执行的内部任务，不经过消息通道排队。
pub(super) enum ScheduledWorkerTask {
    MetadataSettle(u64),
    VolumeRebind { target_id: u64, attempt: usize },
}

#[derive(Clone, Copy)]
struct VolumeRebindDeadline {
    target_id: u64,
    attempt: usize,
    due_at: Instant,
}

/// 归并并持有 media worker 的短期截止时间，避免为延迟刷新创建线程。
#[derive(Default)]
pub(super) struct WorkerDeadlines {
    metadata_settle: HashMap<u64, Instant>,
    volume_rebind: Option<VolumeRebindDeadline>,
}

impl WorkerDeadlines {
    /// 将同一会话的元数据稳定刷新合并到最后一次事件之后。
    pub(super) fn schedule_metadata_settle(
        &mut self,
        session_id: u64,
        observed_at: Instant,
    ) -> bool {
        let due_at = observed_at + METADATA_SETTLE_DELAY;
        match self.metadata_settle.entry(session_id) {
            Entry::Occupied(mut entry) => {
                if due_at > *entry.get() {
                    entry.insert(due_at);
                }
                true
            }
            Entry::Vacant(entry) => {
                entry.insert(due_at);
                false
            }
        }
    }

    /// 返回当前等待稳定刷新的会话数量。
    pub(super) fn metadata_settle_pending_count(&self) -> usize {
        self.metadata_settle.len()
    }

    /// 安排当前音量目标的指定退避重绑，切换目标时覆盖旧任务。
    pub(super) fn schedule_volume_rebind(&mut self, target_id: u64, attempt: usize) {
        self.volume_rebind =
            INITIAL_VOLUME_REBIND_DELAYS
                .get(attempt)
                .map(|delay| VolumeRebindDeadline {
                    target_id,
                    attempt,
                    due_at: Instant::now() + *delay,
                });
    }

    /// 音频会话已就绪或媒体目标已清空时取消剩余重绑。
    pub(super) fn cancel_volume_rebind(&mut self) {
        self.volume_rebind = None;
    }

    /// 返回距离最早任务到期的时间；无任务时由 worker 无限等待消息。
    pub(super) fn next_timeout(&self, now: Instant) -> Option<Duration> {
        self.metadata_settle
            .values()
            .copied()
            .chain(self.volume_rebind.map(|deadline| deadline.due_at))
            .min()
            .map(|due_at| due_at.saturating_duration_since(now))
    }

    /// 一次取出当前全部到期任务，未到期任务继续保留。
    pub(super) fn take_due(&mut self, now: Instant) -> Vec<ScheduledWorkerTask> {
        let mut tasks = Vec::new();
        self.metadata_settle.retain(|session_id, due_at| {
            if *due_at <= now {
                tasks.push(ScheduledWorkerTask::MetadataSettle(*session_id));
                false
            } else {
                true
            }
        });
        if self
            .volume_rebind
            .is_some_and(|deadline| deadline.due_at <= now)
            && let Some(deadline) = self.volume_rebind.take()
        {
            tasks.push(ScheduledWorkerTask::VolumeRebind {
                target_id: deadline.target_id,
                attempt: deadline.attempt,
            });
        }
        tasks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 同一会话的重复事件把稳定刷新推迟到最后一次观测之后，而不是每次新建一个截止时间。
    #[test]
    fn metadata_settle_coalesces_to_latest_observation() {
        let mut deadlines = WorkerDeadlines::default();
        let base = Instant::now();

        assert!(!deadlines.schedule_metadata_settle(1, base));
        assert!(deadlines.schedule_metadata_settle(1, base + Duration::from_millis(100)));
        assert_eq!(deadlines.metadata_settle_pending_count(), 1);

        // 到期时间为"最后一次观测 + 稳定延迟"。
        let due_at = base + Duration::from_millis(100) + METADATA_SETTLE_DELAY;
        assert!(
            deadlines
                .take_due(due_at - Duration::from_millis(1))
                .is_empty()
        );
        let due = deadlines.take_due(due_at);
        assert_eq!(due.len(), 1);
        assert!(matches!(due[0], ScheduledWorkerTask::MetadataSettle(1)));
        assert_eq!(deadlines.metadata_settle_pending_count(), 0);
    }

    /// 到期任务必须被取走，否则 worker 会反复执行同一次刷新。
    #[test]
    fn due_tasks_are_removed_after_being_taken() {
        let mut deadlines = WorkerDeadlines::default();
        let base = Instant::now();
        deadlines.schedule_metadata_settle(1, base);
        let due_at = base + METADATA_SETTLE_DELAY;

        assert_eq!(deadlines.take_due(due_at).len(), 1);
        assert!(deadlines.take_due(due_at).is_empty());
        assert!(deadlines.next_timeout(due_at).is_none());
    }

    /// 重绑退避表是有限的：越界后不再安排任务，避免对已经失效的目标无限重试。
    #[test]
    fn volume_rebind_backoff_table_is_bounded() {
        let mut deadlines = WorkerDeadlines::default();

        deadlines.schedule_volume_rebind(7, 0);
        assert!(deadlines.next_timeout(Instant::now()).is_some());

        deadlines.schedule_volume_rebind(7, INITIAL_VOLUME_REBIND_DELAYS.len());
        assert!(deadlines.next_timeout(Instant::now()).is_none());
    }

    /// 刚安排的退避任务不应立刻到期，worker 必须继续等待消息。
    #[test]
    fn freshly_scheduled_rebind_is_not_due_yet() {
        let mut deadlines = WorkerDeadlines::default();
        deadlines.schedule_volume_rebind(7, 0);
        assert!(deadlines.take_due(Instant::now()).is_empty());
    }

    /// 切换音量目标时覆盖旧任务，避免为已经失效的目标做重试。
    #[test]
    fn canceling_volume_rebind_drops_the_deadline() {
        let mut deadlines = WorkerDeadlines::default();
        deadlines.schedule_volume_rebind(7, 0);
        deadlines.cancel_volume_rebind();
        assert!(deadlines.next_timeout(Instant::now()).is_none());
    }

    #[test]
    fn empty_deadlines_never_time_out() {
        assert!(
            WorkerDeadlines::default()
                .next_timeout(Instant::now())
                .is_none()
        );
    }
}
