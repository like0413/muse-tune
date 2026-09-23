//! 歌词解析、缓存与适配器的诊断传输模型。

use crate::media::MediaPlayer;

use super::{
    LyricsOnlineStrategy, LyricsPrecision, LyricsResolutionMethod, LyricsResolutionOutcome,
    LyricsResolutionSite, LyricsSource, LyricsStatus,
};

/// 最近一次解析的有界步骤记录，仅保留诊断所需摘要。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResolutionStep {
    pub site: LyricsResolutionSite,
    pub outcome: LyricsResolutionOutcome,
    pub detail: Option<String>,
    /// 该步骤属于并发查询多个来源的阶段。
    pub parallel: bool,
}

/// Muse Tune 规范化歌词缓存的磁盘状态。
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsCacheDiagnostics {
    pub schema_version: String,
    pub entry_count: usize,
    pub total_bytes: u64,
    pub limit_bytes: u64,
    pub current_entry_exists: bool,
    pub current_entry_bytes: Option<u64>,
    pub current_entry_age_seconds: Option<u64>,
    pub current_entry_fresh: Option<bool>,
    pub current_refresh_remaining_seconds: Option<u64>,
}

/// 单个播放器歌词适配器的自动发现与监听状态。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsAdapterDiagnostics {
    pub player: MediaPlayer,
    pub cache_path: Option<String>,
    pub cache_path_available: bool,
    pub watcher_active: bool,
}

/// 设置页只读展示的歌词运行状态，不开放路径覆盖能力。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsDiagnostics {
    pub snapshot: LyricsSnapshotDiagnostics,
    pub current_player: Option<MediaPlayer>,
    pub enabled: bool,
    pub online_strategy: LyricsOnlineStrategy,
    pub resolution_method: LyricsResolutionMethod,
    pub local_cache_path: Option<String>,
    pub local_cache_available: bool,
    pub resolver_running: bool,
    pub pending_resolution: bool,
    pub resolution_duration_ms: Option<u64>,
    pub resolution_track: Option<LyricsResolutionTrack>,
    pub resolution_steps: Vec<LyricsResolutionStep>,
    pub recent_resolutions: Vec<LyricsResolutionRecord>,
    pub cache: LyricsCacheDiagnostics,
    pub adapters: Vec<LyricsAdapterDiagnostics>,
}

/// 一次已结束解析的完整记录：尝试过哪些来源，以及最终结论。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResolutionRecord {
    pub finished_at_seconds: Option<u64>,
    pub duration_ms: Option<u64>,
    pub track: Option<LyricsResolutionTrack>,
    pub steps: Vec<LyricsResolutionStep>,
    pub status: LyricsStatus,
    pub source: Option<LyricsSource>,
    pub precision: Option<LyricsPrecision>,
    pub error_reason: Option<String>,
}

/// 一轮解析所使用的曲目标识字段；与来源匹配依据保持一致。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResolutionTrack {
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<u64>,
}

/// 歌词诊断只传递摘要，避免正文和逐字数组进入 IPC。
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSnapshotDiagnostics {
    pub status: LyricsStatus,
    pub source: Option<LyricsSource>,
    pub precision: Option<LyricsPrecision>,
    pub line_count: usize,
    pub error_reason: Option<String>,
}
