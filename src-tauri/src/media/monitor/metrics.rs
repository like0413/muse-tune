use std::{
    array,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Instant,
};

use crate::media::{MediaWorkerMessageRuntimeDiagnostics, MediaWorkerRuntimeDiagnostics};

use super::WorkerMessage;

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
    TogglePlayerWindow,
    GetVolume,
    SetVolume,
    ToggleMute,
    GetSystemVolume,
    SetSystemVolume,
    ToggleSystemMute,
    GetDiagnostics,
    SpectrumEnabled,
    VolumeChanged,
    VolumeSessionsChanged,
    SystemVolumeChanged,
    DefaultAudioEndpointChanged,
    Shutdown,
}

impl WorkerMessageKind {
    const ALL: [Self; 20] = [
        Self::ManagerChanged,
        Self::MediaPropertiesChanged,
        Self::PlaybackInfoChanged,
        Self::TimelinePropertiesChanged,
        Self::SelectionPolicyChanged,
        Self::Control,
        Self::TogglePlayerWindow,
        Self::GetVolume,
        Self::SetVolume,
        Self::ToggleMute,
        Self::GetSystemVolume,
        Self::SetSystemVolume,
        Self::ToggleSystemMute,
        Self::GetDiagnostics,
        Self::SpectrumEnabled,
        Self::VolumeChanged,
        Self::VolumeSessionsChanged,
        Self::SystemVolumeChanged,
        Self::DefaultAudioEndpointChanged,
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
            Self::TogglePlayerWindow => "toggle_player_window",
            Self::GetVolume => "get_volume",
            Self::SetVolume => "set_volume",
            Self::ToggleMute => "toggle_mute",
            Self::GetSystemVolume => "get_system_volume",
            Self::SetSystemVolume => "set_system_volume",
            Self::ToggleSystemMute => "toggle_system_mute",
            Self::GetDiagnostics => "get_diagnostics",
            Self::SpectrumEnabled => "spectrum_enabled",
            Self::VolumeChanged => "volume_changed",
            Self::VolumeSessionsChanged => "volume_sessions_changed",
            Self::SystemVolumeChanged => "system_volume_changed",
            Self::DefaultAudioEndpointChanged => "default_audio_endpoint_changed",
            Self::Shutdown => "shutdown",
        }
    }

    /// 只有需要同步响应的用户或诊断请求计入 command 排队时间。
    const fn is_command(self) -> bool {
        matches!(
            self,
            Self::SelectionPolicyChanged
                | Self::Control
                | Self::TogglePlayerWindow
                | Self::GetVolume
                | Self::SetVolume
                | Self::ToggleMute
                | Self::GetSystemVolume
                | Self::SetSystemVolume
                | Self::ToggleSystemMute
                | Self::GetDiagnostics
                | Self::SpectrumEnabled
        )
    }

    pub(super) const fn index(self) -> usize {
        self as usize
    }
}

impl WorkerMessage {
    /// 将消息映射到低基数诊断分类，避免保存逐事件日志。
    pub(super) fn kind(&self) -> Option<WorkerMessageKind> {
        Some(match self {
            // 事件 wake 只负责提示 worker 读取 latest-state，不属于可靠消息统计。
            Self::EventsReady => return None,
            Self::SelectionPolicyChanged(_, _) => WorkerMessageKind::SelectionPolicyChanged,
            Self::Control(_, _) => WorkerMessageKind::Control,
            Self::TogglePlayerWindow(_) => WorkerMessageKind::TogglePlayerWindow,
            Self::GetVolume(_) => WorkerMessageKind::GetVolume,
            Self::SetVolume(_, _) => WorkerMessageKind::SetVolume,
            Self::ToggleMute(_) => WorkerMessageKind::ToggleMute,
            Self::GetSystemVolume(_) => WorkerMessageKind::GetSystemVolume,
            Self::SetSystemVolume(_, _) => WorkerMessageKind::SetSystemVolume,
            Self::ToggleSystemMute(_) => WorkerMessageKind::ToggleSystemMute,
            Self::GetDiagnostics(_) => WorkerMessageKind::GetDiagnostics,
            Self::SpectrumEnabled(_, _, _) => WorkerMessageKind::SpectrumEnabled,
            Self::Shutdown => WorkerMessageKind::Shutdown,
        })
    }
}

pub(super) struct ChannelMetrics {
    pub(super) sent: [AtomicU64; MESSAGE_KIND_COUNT],
    pub(super) pending: AtomicUsize,
    pub(super) pending_peak: AtomicUsize,
    pub(super) coalesced_events: AtomicU64,
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

/// 带入队时间和分类的内部消息信封。
pub(super) struct WorkerEnvelope {
    pub(super) message: WorkerMessage,
    pub(super) kind: Option<WorkerMessageKind>,
    pub(super) enqueued_at: Instant,
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
    /// 使用通道共享计数创建 worker 独占指标。
    pub(super) fn new(channel: Arc<ChannelMetrics>) -> Self {
        Self {
            channel,
            processed: [0; MESSAGE_KIND_COUNT],
            coalesced_event_count: 0,
            max_command_queue_wait_ms: 0,
            metadata_settle_pending_peak: 0,
        }
    }

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
