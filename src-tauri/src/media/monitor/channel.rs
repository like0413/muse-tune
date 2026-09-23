//! media worker 的消息通道与可合并事件唤醒。

use std::{
    sync::{
        Arc, Mutex,
        atomic::Ordering,
        mpsc::{self, RecvError, RecvTimeoutError, SendError},
    },
    time::{Duration, Instant},
};

use super::{
    WorkerMessage,
    metrics::{ChannelMetrics, WorkerEnvelope, WorkerMetrics},
    pending_events::{PendingWorkerEvents, WorkerEvent, WorkerEventBatch},
};

/// 所有 media 消息生产者共享的 sender；不改变无界通道的投递语义。
#[derive(Clone)]
pub(in crate::media) struct WorkerSender {
    sender: mpsc::Sender<WorkerEnvelope>,
    metrics: Arc<ChannelMetrics>,
    pending_events: Arc<Mutex<PendingWorkerEvents>>,
}

impl WorkerSender {
    /// 记录成功入队的消息类型、当前 pending 与历史峰值。
    pub(in crate::media) fn send(
        &self,
        message: WorkerMessage,
    ) -> Result<(), SendError<WorkerMessage>> {
        let kind = message.kind();
        if let Some(kind) = kind {
            self.metrics.sent[kind.index()].fetch_add(1, Ordering::Relaxed);
        }
        self.send_envelope(message, kind)
    }

    /// 把可合并通知写入 latest-state，并保证通道中至多有一个事件 wake。
    pub(in crate::media) fn send_event(&self, event: WorkerEvent) {
        let kind = event.kind();
        self.metrics.sent[kind.index()].fetch_add(1, Ordering::Relaxed);
        let should_wake = {
            let mut pending = self
                .pending_events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if pending.merge(event) {
                self.metrics
                    .coalesced_events
                    .fetch_add(1, Ordering::Relaxed);
            }
            if pending.wake_enqueued {
                false
            } else {
                pending.wake_enqueued = true;
                true
            }
        };
        if should_wake
            && self
                .send_envelope(WorkerMessage::EventsReady, None)
                .is_err()
        {
            let mut pending = self
                .pending_events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            pending.wake_enqueued = false;
        }
    }

    /// 取出一次 wake 覆盖的全部逻辑通知；锁内同时开放下一次 wake，避免丢失竞态。
    pub(super) fn take_pending_events(&self) -> WorkerEventBatch {
        self.pending_events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take_batch()
    }

    fn send_envelope(
        &self,
        message: WorkerMessage,
        kind: Option<super::metrics::WorkerMessageKind>,
    ) -> Result<(), SendError<WorkerMessage>> {
        let pending = self.metrics.pending.fetch_add(1, Ordering::Relaxed) + 1;
        self.metrics
            .pending_peak
            .fetch_max(pending, Ordering::Relaxed);
        let envelope = WorkerEnvelope {
            message,
            kind,
            enqueued_at: Instant::now(),
        };
        self.sender.send(envelope).map_err(|error| {
            if let Some(kind) = kind {
                self.metrics.sent[kind.index()].fetch_sub(1, Ordering::Relaxed);
            }
            self.metrics.pending.fetch_sub(1, Ordering::Relaxed);
            SendError(error.0.message)
        })
    }
}

/// media worker 独占的 receiver，取出消息时同步扣减 pending。
pub(super) struct WorkerReceiver {
    receiver: mpsc::Receiver<WorkerEnvelope>,
    metrics: Arc<ChannelMetrics>,
}

impl WorkerReceiver {
    pub(super) fn recv(&self) -> Result<WorkerEnvelope, RecvError> {
        self.receiver.recv().inspect(|_| self.record_dequeued())
    }

    pub(super) fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<WorkerEnvelope, RecvTimeoutError> {
        self.receiver
            .recv_timeout(timeout)
            .inspect(|_| self.record_dequeued())
    }

    fn record_dequeued(&self) {
        self.metrics.pending.fetch_sub(1, Ordering::Relaxed);
    }
}

/// 创建保持原有可靠语义的无界 media 通道，并附带常量空间观测状态。
pub(super) fn channel() -> (WorkerSender, WorkerReceiver, WorkerMetrics) {
    let (sender, receiver) = mpsc::channel();
    let metrics = Arc::new(ChannelMetrics::default());
    let pending_events = Arc::new(Mutex::new(PendingWorkerEvents::default()));
    (
        WorkerSender {
            sender,
            metrics: Arc::clone(&metrics),
            pending_events,
        },
        WorkerReceiver {
            receiver,
            metrics: Arc::clone(&metrics),
        },
        WorkerMetrics::new(metrics),
    )
}
