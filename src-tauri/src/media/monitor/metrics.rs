use std::{
    array,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, RecvError, RecvTimeoutError, SendError},
    },
    time::{Duration, Instant},
};

use crate::media::{MediaWorkerMessageRuntimeDiagnostics, MediaWorkerRuntimeDiagnostics};

use super::{
    WorkerMessage,
    pending_events::{PendingWorkerEvents, WorkerEvent, WorkerEventBatch},
};

const MESSAGE_KIND_COUNT: usize = WorkerMessageKind::ALL.len();

/// media worker 通道中的稳定消息分类，用于按类型比较生产量与消费量。
#[derive(Clone, Copy)]
pub(super) enum WorkerMessageKind {
    ManagerChanged,
    MediaPropertiesChanged,
    PlaybackInfoChanged,
    TimelinePropertiesChanged,
    SelectionPolicyChanged,
    Control,
    GetVolume,
    SetVolume,
    ToggleMute,
    GetDiagnostics,
    SpectrumEnabled,
    VolumeChanged,
    VolumeSessionsChanged,
    Shutdown,
}

impl WorkerMessageKind {
    const ALL: [Self; 14] = [
        Self::ManagerChanged,
        Self::MediaPropertiesChanged,
        Self::PlaybackInfoChanged,
        Self::TimelinePropertiesChanged,
        Self::SelectionPolicyChanged,
        Self::Control,
        Self::GetVolume,
        Self::SetVolume,
        Self::ToggleMute,
        Self::GetDiagnostics,
        Self::SpectrumEnabled,
        Self::VolumeChanged,
        Self::VolumeSessionsChanged,
        Self::Shutdown,
    ];

    /// 返回可长期写入诊断报告的稳定名称。
    const fn name(self) -> &'static str {
        match self {
            Self::ManagerChanged => "manager_changed",
            Self::MediaPropertiesChanged => "media_properties_changed",
            Self::PlaybackInfoChanged => "playback_info_changed",
            Self::TimelinePropertiesChanged => "timeline_properties_changed",
            Self::SelectionPolicyChanged => "selection_policy_changed",
            Self::Control => "control",
            Self::GetVolume => "get_volume",
            Self::SetVolume => "set_volume",
            Self::ToggleMute => "toggle_mute",
            Self::GetDiagnostics => "get_diagnostics",
            Self::SpectrumEnabled => "spectrum_enabled",
            Self::VolumeChanged => "volume_changed",
            Self::VolumeSessionsChanged => "volume_sessions_changed",
            Self::Shutdown => "shutdown",
        }
    }

    /// 只有需要同步响应的用户或诊断请求计入 command 排队时间。
    const fn is_command(self) -> bool {
        matches!(
            self,
            Self::SelectionPolicyChanged
                | Self::Control
                | Self::GetVolume
                | Self::SetVolume
                | Self::ToggleMute
                | Self::GetDiagnostics
                | Self::SpectrumEnabled
        )
    }

    const fn index(self) -> usize {
        self as usize
    }
}

impl WorkerMessage {
    /// 将消息映射到低基数诊断分类，避免保存逐事件日志。
    fn kind(&self) -> WorkerMessageKind {
        match self {
            Self::EventsReady => unreachable!("事件 wake 不属于可靠 command 分类"),
            Self::SelectionPolicyChanged(_, _) => WorkerMessageKind::SelectionPolicyChanged,
            Self::Control(_, _) => WorkerMessageKind::Control,
            Self::GetVolume(_) => WorkerMessageKind::GetVolume,
            Self::SetVolume(_, _) => WorkerMessageKind::SetVolume,
            Self::ToggleMute(_) => WorkerMessageKind::ToggleMute,
            Self::GetDiagnostics(_) => WorkerMessageKind::GetDiagnostics,
            Self::SpectrumEnabled(_, _) => WorkerMessageKind::SpectrumEnabled,
            Self::Shutdown => WorkerMessageKind::Shutdown,
        }
    }
}

struct ChannelMetrics {
    sent: [AtomicU64; MESSAGE_KIND_COUNT],
    pending: AtomicUsize,
    pending_peak: AtomicUsize,
    coalesced_events: AtomicU64,
}

impl Default for ChannelMetrics {
    fn default() -> Self {
        Self {
            sent: array::from_fn(|_| AtomicU64::new(0)),
            pending: AtomicUsize::new(0),
            pending_peak: AtomicUsize::new(0),
            coalesced_events: AtomicU64::new(0),
        }
    }
}

/// 所有 media 消息生产者共享的计数 sender；不改变无界通道的投递语义。
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
        self.metrics.sent[kind.index()].fetch_add(1, Ordering::Relaxed);
        self.send_envelope(message, Some(kind))
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
        kind: Option<WorkerMessageKind>,
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

/// 带入队时间和分类的内部消息信封。
pub(super) struct WorkerEnvelope {
    message: WorkerMessage,
    kind: Option<WorkerMessageKind>,
    enqueued_at: Instant,
}

impl WorkerEnvelope {
    pub(super) fn into_message(self) -> WorkerMessage {
        self.message
    }
}

/// media worker 独占的累计观测状态，不需要锁或额外原子操作。
pub(super) struct WorkerMetrics {
    channel: Arc<ChannelMetrics>,
    processed: [u64; MESSAGE_KIND_COUNT],
    coalesced_event_count: u64,
    max_command_queue_wait_ms: u64,
    metadata_settle_pending_peak: usize,
}

impl WorkerMetrics {
    /// 在 worker 取到消息后记录消费量和 command 排队时长。
    pub(super) fn record_received(&mut self, envelope: &WorkerEnvelope) {
        let Some(kind) = envelope.kind else {
            return;
        };
        self.processed[kind.index()] = self.processed[kind.index()].saturating_add(1);
        if kind.is_command() {
            let wait_ms =
                u64::try_from(envelope.enqueued_at.elapsed().as_millis()).unwrap_or(u64::MAX);
            self.max_command_queue_wait_ms = self.max_command_queue_wait_ms.max(wait_ms);
        }
    }

    /// 记录一次从 latest-state 实际消费的逻辑通知。
    pub(super) fn record_event_processed(&mut self, kind: WorkerMessageKind) {
        self.processed[kind.index()] = self.processed[kind.index()].saturating_add(1);
    }

    /// 记录被已有 latest-deadline 吸收的重复事件。
    pub(super) fn record_coalesced_event(&mut self) {
        self.coalesced_event_count = self.coalesced_event_count.saturating_add(1);
    }

    /// 更新 metadata settle 同时待处理数量的历史峰值。
    pub(super) fn observe_metadata_settle_pending(&mut self, pending: usize) {
        self.metadata_settle_pending_peak = self.metadata_settle_pending_peak.max(pending);
    }

    /// 生成已有 diagnostics command 使用的轻量快照。
    pub(super) fn snapshot(&self, metadata_settle_pending: usize) -> MediaWorkerRuntimeDiagnostics {
        MediaWorkerRuntimeDiagnostics {
            messages: WorkerMessageKind::ALL
                .into_iter()
                .map(|kind| MediaWorkerMessageRuntimeDiagnostics {
                    kind: kind.name(),
                    sent: self.channel.sent[kind.index()].load(Ordering::Relaxed),
                    processed: self.processed[kind.index()],
                })
                .collect(),
            pending_messages: self.channel.pending.load(Ordering::Relaxed),
            pending_messages_peak: self.channel.pending_peak.load(Ordering::Relaxed),
            coalesced_event_count: self
                .coalesced_event_count
                .saturating_add(self.channel.coalesced_events.load(Ordering::Relaxed)),
            max_command_queue_wait_ms: self.max_command_queue_wait_ms,
            metadata_settle_pending,
            metadata_settle_pending_peak: self.metadata_settle_pending_peak,
        }
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
        WorkerMetrics {
            channel: metrics,
            processed: [0; MESSAGE_KIND_COUNT],
            coalesced_event_count: 0,
            max_command_queue_wait_ms: 0,
            metadata_settle_pending_peak: 0,
        },
    )
}
