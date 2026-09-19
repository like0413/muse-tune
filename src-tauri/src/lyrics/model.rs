use crate::media::MediaPlayer;

use super::track::normalize_text;

const MIN_WORD_TIMING_COVERAGE_PERCENT: usize = 80;

/// 歌词解析生命周期；技术错误与“确实无歌词”保持可区分。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsStatus {
    Loading,
    Ready,
    /// 平台已明确声明当前歌曲为纯音乐，不属于歌词时间轴。
    Instrumental,
    #[default]
    Unavailable,
    Error,
}

/// 歌词时间精度。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsPrecision {
    Word,
    Line,
}

/// 歌词来自播放器缓存还是国内在线接口。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsSourceKind {
    Local,
    Online,
}

/// 当前歌词在本次播放中的实际取得方式，与歌词的原始平台来源分开记录。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsResolutionMethod {
    #[default]
    None,
    ApplicationCache,
    PlayerLocal,
    Online,
}

/// 最近一次歌词解析步骤的结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsResolutionOutcome {
    Hit,
    Miss,
    Error,
}

/// 在线歌词来源的调度策略。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsOnlineStrategy {
    /// 同时查询所有可用来源，缩短兜底等待时间，但会产生更多网络请求。
    #[default]
    Parallel,
    /// 先查询当前播放器，未获得可靠逐字歌词时再并发查询其他来源。
    CurrentPlayerFirst,
}

/// 最近一次解析的有界步骤记录，仅保留诊断所需摘要。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResolutionStep {
    pub label: String,
    pub outcome: LyricsResolutionOutcome,
    pub detail: Option<String>,
    pub parallel_group: Option<String>,
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

/// 可展示且可诊断的歌词来源。
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSource {
    pub player: MediaPlayer,
    pub kind: LyricsSourceKind,
    pub song_id: Option<String>,
}

/// 单个逐字片段的绝对时间范围。
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricWord {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// 统一后的单行歌词。
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub translation: Option<String>,
    pub romanization: Option<String>,
    pub words: Vec<LyricWord>,
}

/// 仅把绝大多数正文行都有完整时间轴的规范化歌词视为逐字结果。
pub fn has_word_timing(lines: &[LyricLine]) -> bool {
    let mut eligible_count = 0usize;
    let mut timed_count = 0usize;
    // 逐行拼接复用同一缓冲，避免为每一行单独分配字符串。
    let mut timed_text = String::new();
    for line in lines {
        let normalized = normalize_text(&line.text);
        if normalized.is_empty() {
            continue;
        }
        eligible_count += 1;
        timed_text.clear();
        for word in &line.words {
            if word.end_ms > word.start_ms && !word.text.trim().is_empty() {
                timed_text.push_str(&word.text);
            }
        }
        if !timed_text.is_empty() && normalize_text(&timed_text) == normalized {
            timed_count += 1;
        }
    }
    eligible_count > 0 && timed_count * 100 >= eligible_count * MIN_WORD_TIMING_COVERAGE_PERCENT
}

/// 独立于媒体快照广播的歌词状态。
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSnapshot {
    pub track_key: Option<String>,
    pub status: LyricsStatus,
    pub source: Option<LyricsSource>,
    pub precision: Option<LyricsPrecision>,
    pub lines: Vec<LyricLine>,
    pub error_reason: Option<String>,
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
    pub resolution_steps: Vec<LyricsResolutionStep>,
    pub cache: LyricsCacheDiagnostics,
    pub adapters: Vec<LyricsAdapterDiagnostics>,
    pub watcher: LyricsWatcherDiagnostics,
}

/// 歌词目录 watcher 的累计背压指标，不包含任何实际路径或文件名。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsWatcherDiagnostics {
    pub enqueued_batches: u64,
    pub processed_batches: u64,
    pub coalesced_batches: u64,
    pub callback_count: u64,
    pub pending_batches: usize,
    pub pending_batches_peak: usize,
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

impl LyricsSnapshot {
    /// 创建不携带旧歌词的加载状态。
    pub fn loading(track_key: String) -> Self {
        Self {
            track_key: Some(track_key),
            status: LyricsStatus::Loading,
            ..Self::default()
        }
    }

    /// 创建已解析状态；平台纯音乐说明转换为不携带时间轴的语义状态。
    pub fn from_resolved(track_key: String, resolved: ResolvedLyrics) -> Self {
        let ResolvedLyrics { source, lines } = resolved;
        if is_instrumental_notice(&lines) {
            return Self {
                track_key: Some(track_key),
                status: LyricsStatus::Instrumental,
                source: Some(source),
                ..Self::default()
            };
        }
        let precision = if has_word_timing(&lines) {
            LyricsPrecision::Word
        } else {
            LyricsPrecision::Line
        };
        Self {
            track_key: Some(track_key),
            status: LyricsStatus::Ready,
            source: Some(source),
            precision: Some(precision),
            lines,
            error_reason: None,
        }
    }

    /// 创建当前歌曲没有可靠歌词的可恢复状态。
    pub fn unavailable(track_key: Option<String>, reason: impl Into<String>) -> Self {
        Self {
            track_key,
            error_reason: Some(reason.into()),
            ..Self::default()
        }
    }
}

/// 播放器适配器返回的内部统一结果。
#[derive(Clone, Debug)]
pub struct ResolvedLyrics {
    pub source: LyricsSource,
    pub lines: Vec<LyricLine>,
}

/// 歌词适配器对协调器返回的统一业务结果；技术失败继续由 `LyricsError` 表达。
#[derive(Debug)]
pub enum LyricsLookupOutcome {
    /// 当前适配器不具备所请求的能力。
    Unsupported,
    /// 适配器具备能力，但本次没有找到可靠歌词。
    Miss(LyricsLookupMiss),
    /// 找到可进入统一质量校验的歌词候选。
    Hit(ResolvedLyrics),
}

/// 正常未命中的稳定分类，供编排和诊断使用，不承载平台私有细节。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LyricsLookupMiss {
    /// 解析所需的播放器缓存、索引或歌曲元数据尚不可用。
    DataUnavailable,
    /// 适配器已执行，但没有产出通过其内部规则的可靠歌词。
    NoReliableLyrics,
}

/// 仅识别已确认的平台固定文案，避免把普通歌词中的“纯音乐”误判为语义状态。
fn is_instrumental_notice(lines: &[LyricLine]) -> bool {
    let [line] = lines else {
        return false;
    };
    matches!(
        normalize_text(&line.text).as_str(),
        "此歌曲为没有填词的纯音乐请您欣赏" | "此歌曲为没有填词的纯音乐请您欣" | "纯音乐请欣赏"
    )
}
