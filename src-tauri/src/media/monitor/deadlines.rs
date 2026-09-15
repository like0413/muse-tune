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
