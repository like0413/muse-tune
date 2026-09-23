use crate::media::MediaPlayer;

mod diagnostics;

pub use diagnostics::*;

use super::{notice::platform_notice, track::normalize_text};

const MIN_WORD_TIMING_COVERAGE_PERCENT: usize = 80;

/// 歌词解析生命周期；技术错误与“确实无歌词”保持可区分。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsStatus {
    Loading,
    Ready,
    /// 平台已明确声明当前歌曲为纯音乐，不属于歌词时间轴。
    Instrumental,
    /// 平台已明确声明当前歌曲没有歌词；与纯音乐、技术失败都是不同结论。
    NoLyrics,
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
    /// 并行查询用户勾选的在线接口，按用户排定的顺序发起。
    #[default]
    Parallel,
    /// 只查询当前正在播放的平台自己的在线接口，不使用其他平台兜底。
    CurrentPlayerOnly,
}

/// 歌词中文字形的实际输出目标；`Original` 表示保留来源原文。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsChineseVariant {
    #[default]
    Original,
    Simplified,
    Traditional,
}

/// 解析步骤的来源标识。
///
/// 这里只输出稳定的机器键，展示文案由前端按语言组装：Rust 直接产出中文会让界面文案无法
/// 跟随语言设置，也让前端只能靠字符串相等来判断步骤含义，改动文案即静默失效。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsResolutionSite {
    /// 应用自身的解析结果缓存。
    ApplicationCache,
    /// 命中缓存后按当前设置原位改写简繁字形。
    ApplicationCacheVariant,
    /// 当前播放器的本地歌词。
    Local,
    /// 当前播放器的在线歌词。
    Online,
    /// 备用平台的在线歌词。
    OnlineFallback,
    /// 已有可展示缓存时在后台尝试的本地精度升级。
    LocalUpgrade,
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

impl LyricsSnapshot {
    /// 创建不携带旧歌词的加载状态。
    pub fn loading(track_key: String) -> Self {
        Self {
            track_key: Some(track_key),
            status: LyricsStatus::Loading,
            ..Self::default()
        }
    }

    /// 创建已解析状态；平台占位文案转换为不携带时间轴的语义状态。
    pub fn from_resolved(track_key: String, resolved: ResolvedLyrics) -> Self {
        let ResolvedLyrics { source, lines } = resolved;
        if let Some(notice) = platform_notice(&lines) {
            return Self {
                track_key: Some(track_key),
                status: notice.status(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> LyricLine {
        LyricLine {
            start_ms: 0,
            end_ms: 1_000,
            text: text.to_owned(),
            translation: None,
            romanization: None,
            words: Vec::new(),
        }
    }

    fn word_line(text: &str) -> LyricLine {
        LyricLine {
            words: vec![LyricWord {
                start_ms: 0,
                end_ms: 1_000,
                text: text.to_owned(),
            }],
            ..line(text)
        }
    }

    /// 只有绝大多数行都有完整逐字覆盖才算逐字歌词，否则前端高亮会大面积缺失。
    #[test]
    fn word_timing_requires_high_coverage() {
        assert!(has_word_timing(&[word_line("第一句"), word_line("第二句")]));

        // 5 行中 4 行有逐字 = 80%，正好达到阈值。
        assert!(has_word_timing(&[
            word_line("第一句"),
            word_line("第二句"),
            word_line("第三句"),
            word_line("第四句"),
            line("第五句"),
        ]));

        assert!(!has_word_timing(&[
            word_line("第一句"),
            word_line("第二句"),
            word_line("第三句"),
            line("第四句"),
            line("第五句"),
        ]));
    }

    #[test]
    fn word_timing_ignores_empty_lines() {
        // 空文本行不计入分母，否则间奏多的歌词永远达不到阈值。
        assert!(has_word_timing(&[word_line("第一句"), line("   ")]));
        assert!(!has_word_timing(&[]));
    }

    /// 单词文本拼起来与整行不一致时不算覆盖，避免错位的逐字数据被当成逐字歌词。
    #[test]
    fn word_timing_requires_matching_text() {
        let mismatched = LyricLine {
            words: vec![LyricWord {
                start_ms: 0,
                end_ms: 1_000,
                text: "别的".to_owned(),
            }],
            ..line("第一句")
        };
        assert!(!has_word_timing(&[mismatched]));
    }
}
