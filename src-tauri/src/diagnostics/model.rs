use crate::{
    lyrics::LyricsDiagnostics,
    media::{
        MediaPlaybackStatus, MediaPlayer, MediaRuntimeDiagnostics, MediaSessionSelectionStrategy,
        MediaSnapshotDiagnostics,
    },
    taskbar::TaskbarDisplay,
};

/// 设置页一次读取的完整应用诊断。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSnapshot {
    pub application: ApplicationDiagnostics,
    pub taskbar: TaskbarDiagnostics,
    pub media: MediaDiagnostics,
    pub lyrics: LyricsDiagnostics,
    pub storage: StorageDiagnostics,
    pub issues: Vec<DiagnosticIssue>,
}

/// 不包含设备标识的应用与运行环境信息。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationDiagnostics {
    pub name: String,
    pub version: String,
    pub build_profile: String,
    pub target_arch: String,
    pub target_os: String,
    pub cache_directory: Option<String>,
    pub log_directory: Option<String>,
}

/// 当前任务栏集成的轻量运行状态。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarDiagnostics {
    pub detected_display_count: usize,
    pub bar_window_count: usize,
    pub visible_bar_window_count: usize,
    pub content_visible: bool,
    pub display_target: String,
    pub placement: String,
    pub overlap_priority: String,
    pub content_width_dip: i32,
    pub width_mode: String,
    pub displays: Vec<TaskbarDisplay>,
    pub windows: Vec<TaskbarWindowDiagnostics>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarWindowDiagnostics {
    pub label: String,
    pub visible: bool,
}

/// 从 GSMTC 快照提取的诊断字段，不传输封面数据。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDiagnostics {
    pub session_available: bool,
    pub player: Option<MediaPlayer>,
    pub playback_status: Option<MediaPlaybackStatus>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub timeline_available: bool,
    pub timeline_start_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub playback_rate: Option<f64>,
    pub can_seek: bool,
    pub can_toggle_play_pause: bool,
    pub can_skip_next: bool,
    pub can_skip_previous: bool,
    pub discovered_session_count: Option<usize>,
    pub selection_strategy: Option<String>,
    pub audio_session_bound: bool,
    pub audio_process_id: Option<u32>,
    pub volume_level: Option<f32>,
    pub muted: Option<bool>,
    pub spectrum_enabled: Option<bool>,
    pub spectrum_active: Option<bool>,
    pub worker: Option<MediaWorkerDiagnostics>,
    pub runtime_error: Option<String>,
    pub sessions: Vec<MediaSessionDiagnostics>,
}

/// media worker 的累计背压观测；所有计数均从本次进程启动开始。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaWorkerDiagnostics {
    pub messages: Vec<MediaWorkerMessageDiagnostics>,
    pub pending_messages: usize,
    pub pending_messages_peak: usize,
    pub coalesced_event_count: u64,
    pub max_command_queue_wait_ms: u64,
    pub metadata_settle_pending: usize,
    pub metadata_settle_pending_peak: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaWorkerMessageDiagnostics {
    pub kind: &'static str,
    pub sent: u64,
    pub processed: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSessionDiagnostics {
    pub player: MediaPlayer,
    pub playback_status: MediaPlaybackStatus,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub timeline_available: bool,
    pub selected: bool,
}

impl MediaDiagnostics {
    pub(super) fn from_snapshots(
        snapshot: Option<MediaSnapshotDiagnostics>,
        runtime: Result<MediaRuntimeDiagnostics, String>,
    ) -> Self {
        let runtime_error = runtime.as_ref().err().cloned();
        let (
            discovered_session_count,
            selection_strategy,
            audio_process_id,
            volume_level,
            muted,
            spectrum_enabled,
            spectrum_active,
            worker,
            sessions,
        ) = runtime.map_or(
            (None, None, None, None, None, None, None, None, Vec::new()),
            |runtime| {
                let worker = MediaWorkerDiagnostics {
                    messages: runtime
                        .worker
                        .messages
                        .into_iter()
                        .map(|message| MediaWorkerMessageDiagnostics {
                            kind: message.kind,
                            sent: message.sent,
                            processed: message.processed,
                        })
                        .collect(),
                    pending_messages: runtime.worker.pending_messages,
                    pending_messages_peak: runtime.worker.pending_messages_peak,
                    coalesced_event_count: runtime.worker.coalesced_event_count,
                    max_command_queue_wait_ms: runtime.worker.max_command_queue_wait_ms,
                    metadata_settle_pending: runtime.worker.metadata_settle_pending,
                    metadata_settle_pending_peak: runtime.worker.metadata_settle_pending_peak,
                };
                (
                    Some(runtime.session_count),
                    Some(selection_strategy_label(runtime.selection_strategy).to_owned()),
                    runtime.audio_process_id,
                    runtime.volume.map(|volume| volume.level),
                    runtime.volume.map(|volume| volume.muted),
                    Some(runtime.spectrum_enabled),
                    Some(runtime.spectrum_active),
                    Some(worker),
                    runtime
                        .sessions
                        .into_iter()
                        .map(|session| MediaSessionDiagnostics {
                            player: session.player,
                            playback_status: session.playback_status,
                            title: session.title,
                            artist: session.artist,
                            timeline_available: session.timeline_available,
                            selected: session.selected,
                        })
                        .collect(),
                )
            },
        );
        let Some(snapshot) = snapshot else {
            return Self {
                session_available: false,
                player: None,
                playback_status: None,
                title: None,
                artist: None,
                timeline_available: false,
                timeline_start_ms: None,
                duration_ms: None,
                playback_rate: None,
                can_seek: false,
                can_toggle_play_pause: false,
                can_skip_next: false,
                can_skip_previous: false,
                discovered_session_count,
                selection_strategy,
                audio_session_bound: audio_process_id.is_some(),
                audio_process_id,
                volume_level,
                muted,
                spectrum_enabled,
                spectrum_active,
                worker,
                runtime_error,
                sessions,
            };
        };
        let timeline_start_ms = snapshot
            .timeline
            .as_ref()
            .map(|timeline| timeline.start_time_ms);
        let duration_ms = snapshot
            .timeline
            .as_ref()
            .and_then(|timeline| timeline.end_time_ms.checked_sub(timeline.start_time_ms));
        let playback_rate = snapshot
            .timeline
            .as_ref()
            .map(|timeline| timeline.playback_rate);
        let can_seek = snapshot
            .timeline
            .as_ref()
            .is_some_and(|timeline| timeline.can_seek);
        let controls = snapshot.controls;
        Self {
            session_available: true,
            player: Some(snapshot.player),
            playback_status: Some(snapshot.playback_status),
            title: non_empty(snapshot.title),
            artist: non_empty(snapshot.artist),
            timeline_available: snapshot.timeline.is_some(),
            timeline_start_ms,
            duration_ms,
            playback_rate,
            can_seek,
            can_toggle_play_pause: controls.can_toggle_play_pause,
            can_skip_next: controls.can_skip_next,
            can_skip_previous: controls.can_skip_previous,
            discovered_session_count,
            selection_strategy,
            audio_session_bound: audio_process_id.is_some(),
            audio_process_id,
            volume_level,
            muted,
            spectrum_enabled,
            spectrum_active,
            worker,
            runtime_error,
            sessions,
        }
    }
}

/// 设置、日志等非歌词缓存的存储状态。
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageDiagnostics {
    pub settings_file: Option<String>,
    pub settings_file_exists: bool,
    pub settings_file_bytes: Option<u64>,
    pub log_file_count: usize,
    pub log_total_bytes: u64,
}

/// 当前仍然成立、可以直接指导排查的异常状态。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticIssue {
    pub severity: DiagnosticIssueSeverity,
    pub area: String,
    pub message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticIssueSeverity {
    Warning,
    Error,
}

fn selection_strategy_label(strategy: MediaSessionSelectionStrategy) -> &'static str {
    match strategy {
        MediaSessionSelectionStrategy::FollowWindows => "跟随 Windows",
        MediaSessionSelectionStrategy::RecentPlayback => "最近播放",
        MediaSessionSelectionStrategy::StickyCurrent => "保持当前",
        MediaSessionSelectionStrategy::FixedPriority => "固定优先级",
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}
