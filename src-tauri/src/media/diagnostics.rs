//! 媒体线程与会话的内部诊断模型，不进入公开媒体 API。

use super::{
    MediaPlaybackControls, MediaPlaybackStatus, MediaPlayer, MediaSessionSelectionStrategy,
    MediaTimeline, MediaVolumeSnapshot,
};

/// 不包含封面和来源图标的媒体诊断摘要。
pub(crate) struct MediaSnapshotDiagnostics {
    pub player: MediaPlayer,
    pub playback_status: MediaPlaybackStatus,
    pub title: String,
    pub artist: String,
    pub timeline: Option<MediaTimeline>,
    pub controls: MediaPlaybackControls,
}

/// 媒体工作线程拥有的轻量运行状态，仅供应用诊断汇总。
pub(crate) struct MediaRuntimeDiagnostics {
    pub session_count: usize,
    pub sessions: Vec<MediaRuntimeSessionDiagnostics>,
    pub selection_strategy: MediaSessionSelectionStrategy,
    pub volume: Option<MediaVolumeSnapshot>,
    pub audio_process_id: Option<u32>,
    pub spectrum_enabled: bool,
    pub spectrum_active: bool,
    pub worker: MediaWorkerRuntimeDiagnostics,
}

/// media worker 的有界聚合观测，不保存歌曲、进程或逐事件内容。
pub(crate) struct MediaWorkerRuntimeDiagnostics {
    pub messages: Vec<MediaWorkerMessageRuntimeDiagnostics>,
    pub pending_messages: usize,
    pub pending_messages_peak: usize,
    pub coalesced_event_count: u64,
    pub max_command_queue_wait_ms: u64,
    pub metadata_settle_pending: usize,
    pub metadata_settle_pending_peak: usize,
}

pub(crate) struct MediaWorkerMessageRuntimeDiagnostics {
    pub kind: &'static str,
    pub sent: u64,
    pub processed: u64,
}

pub(crate) struct MediaRuntimeSessionDiagnostics {
    pub player: MediaPlayer,
    pub playback_status: MediaPlaybackStatus,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub timeline_available: bool,
    pub selected: bool,
}
