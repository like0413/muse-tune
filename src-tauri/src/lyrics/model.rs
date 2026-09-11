use crate::media::MediaPlayer;

use super::track::normalize_text;

const MIN_WORD_TIMING_COVERAGE_PERCENT: usize = 80;

/// 歌词解析生命周期；技术错误与“确实无歌词”保持可区分。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsStatus {
    Loading,
    Ready,
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

/// 最近一次解析的有界步骤记录，仅保留诊断所需摘要。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResolutionStep {
    pub label: String,
    pub outcome: LyricsResolutionOutcome,
    pub detail: Option<String>,
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
    let eligible_lines = lines
        .iter()
        .filter(|line| !normalize_text(&line.text).is_empty())
        .collect::<Vec<_>>();
    if eligible_lines.is_empty() {
        return false;
    }
    let timed_lines = eligible_lines
        .iter()
        .filter(|line| {
            let timed_text = line
                .words
                .iter()
                .filter(|word| word.end_ms > word.start_ms && !word.text.trim().is_empty())
                .map(|word| word.text.as_str())
                .collect::<String>();
            !timed_text.is_empty() && normalize_text(&timed_text) == normalize_text(&line.text)
        })
        .count();
    timed_lines * 100 >= eligible_lines.len() * MIN_WORD_TIMING_COVERAGE_PERCENT
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
    pub resolution_method: LyricsResolutionMethod,
    pub local_cache_path: Option<String>,
    pub local_cache_available: bool,
    pub resolver_running: bool,
    pub pending_resolution: bool,
    pub resolution_duration_ms: Option<u64>,
    pub resolution_steps: Vec<LyricsResolutionStep>,
    pub cache: LyricsCacheDiagnostics,
    pub adapters: Vec<LyricsAdapterDiagnostics>,
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

    /// 创建已解析状态并按实际逐字数据声明精度。
    pub fn ready(track_key: String, resolved: ResolvedLyrics) -> Self {
        let precision = if has_word_timing(&resolved.lines) {
            LyricsPrecision::Word
        } else {
            LyricsPrecision::Line
        };
        Self {
            track_key: Some(track_key),
            status: LyricsStatus::Ready,
            source: Some(resolved.source),
            precision: Some(precision),
            lines: resolved.lines,
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

#[cfg(test)]
mod tests {
    use super::{LyricLine, LyricWord, has_word_timing};

    fn line(text: &str, words: Vec<LyricWord>) -> LyricLine {
        LyricLine {
            start_ms: 0,
            end_ms: 1_000,
            text: text.to_owned(),
            translation: None,
            romanization: None,
            words,
        }
    }

    fn word(text: &str) -> LyricWord {
        LyricWord {
            start_ms: 0,
            end_ms: 500,
            text: text.to_owned(),
        }
    }

    #[test]
    fn word_precision_requires_most_lines_to_have_matching_timing() {
        let lines = [
            line("完整", vec![word("完整")]),
            line("缺失一", Vec::new()),
            line("缺失二", Vec::new()),
        ];

        assert!(!has_word_timing(&lines));
    }

    #[test]
    fn word_precision_accepts_punctuation_differences() {
        let lines = [line("Hello, world!", vec![word("Hello world")])];

        assert!(has_word_timing(&lines));
    }

    #[test]
    fn word_precision_rejects_timing_for_different_text() {
        let lines = [line("正确歌词", vec![word("错误歌词")])];

        assert!(!has_word_timing(&lines));
    }
}
