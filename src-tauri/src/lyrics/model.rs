use crate::media::MediaPlayer;

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

/// 仅把包含真实单字时间范围的规范化歌词视为逐字结果。
pub fn has_word_timing(lines: &[LyricLine]) -> bool {
    lines.iter().any(|line| {
        line.words
            .iter()
            .any(|word| word.end_ms > word.start_ms && !word.text.is_empty())
    })
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
