use std::panic::{AssertUnwindSafe, catch_unwind};

use lyrics_core::{LineInfo, LyricsData, SyllableInfo};
use lyrics_parsers::parsers::{krc_parser, lrc_parser, qrc_parser, yrc_parser};

use super::{
    error::LyricsError,
    model::{LyricLine, LyricWord, notice_text_without_timeline},
};

const DEFAULT_LINE_DURATION_MS: u64 = 5_000;

/// 回填 `end_ms` 时的时长上限。行间隔常常包含间奏，直接取下一行起点会把逐字渐变与横向
/// 滚动拉长到整段间隔；超过上限即按上限收尾。行的激活仍由下一行起点决定，所以间奏期间
/// 该行继续显示，只是行内进度不再被拖满。
const MAX_ESTIMATED_LINE_DURATION_MS: u64 = 8_000;

const AUXILIARY_TIME_TOLERANCE_MS: u64 = 150;

/// 安全解析 LRC，并规避上游解析器对结尾空行的整数下溢。
pub fn parse_lrc_lines(input: &str) -> Result<Vec<LyricLine>, LyricsError> {
    parse_lines("LRC", input, lrc_parser::parse)
}

/// 安全解析 QRC 逐字歌词。
pub fn parse_qrc_lines(input: &str) -> Result<Vec<LyricLine>, LyricsError> {
    parse_lines("QRC", input, qrc_parser::parse)
}

/// 安全解析网易云 YRC 逐字歌词。
pub fn parse_yrc_lines(input: &str) -> Result<Vec<LyricLine>, LyricsError> {
    parse_lines("YRC", input, yrc_parser::parse)
}

/// 安全解析酷狗 KRC 逐字歌词与语言扩展。
pub fn parse_krc_lines(input: &str) -> Result<Vec<LyricLine>, LyricsError> {
    parse_lines("KRC", input, krc_parser::parse)
}

/// 清理尾部换行并隔离第三方解析器 panic，避免解析线程永久停在加载中。
fn parse_lines(
    format: &str,
    input: &str,
    parser: fn(&str) -> LyricsData,
) -> Result<Vec<LyricLine>, LyricsError> {
    let sanitized = input.trim_end_matches(['\r', '\n']);
    if sanitized.is_empty() {
        return Ok(Vec::new());
    }
    let parsed = catch_unwind(AssertUnwindSafe(|| parser(sanitized)))
        .map_err(|_| LyricsError::InvalidData(format!("第三方 {format} 解析器发生异常")))?;
    let lines = normalize_parsed_lines(parsed);
    if lines.is_empty()
        && let Some(text) = notice_text_without_timeline(sanitized)
    {
        // 纯音乐占位文案不带任何时间戳，会被逐行解析整段丢弃，必须在这里补一行承载它；
        // 下游的占位识别随后会把它转成不携带时间轴的语义结论。
        return Ok(vec![LyricLine {
            start_ms: 0,
            end_ms: DEFAULT_LINE_DURATION_MS,
            text,
            translation: None,
            romanization: None,
            words: Vec::new(),
        }]);
    }
    Ok(lines)
}

/// 把第三方解析库的数据收敛为 Muse Tune 的稳定模型。
pub fn normalize_parsed_lines(data: LyricsData) -> Vec<LyricLine> {
    let mut lines = data
        .lines
        .unwrap_or_default()
        .into_iter()
        .filter_map(convert_line)
        .collect::<Vec<_>>();
    lines.sort_unstable_by_key(|line| line.start_ms);
    for index in 0..lines.len() {
        if lines[index].end_ms > lines[index].start_ms {
            continue;
        }
        let estimated_end_ms = lines.get(index + 1).map_or_else(
            || {
                lines[index]
                    .start_ms
                    .saturating_add(DEFAULT_LINE_DURATION_MS)
            },
            |next| next.start_ms.max(lines[index].start_ms.saturating_add(1)),
        );
        // 行间隔常常包含间奏：直接取下一行起点会让逐字渐变与横向滚动被拉长到整段间隔。
        // 超过上限时按上限收尾；行的激活仍由下一行起点决定，所以间奏期间它继续显示。
        lines[index].end_ms = estimated_end_ms.min(
            lines[index]
                .start_ms
                .saturating_add(MAX_ESTIMATED_LINE_DURATION_MS),
        );
    }
    lines
}

/// 拒绝空文本和歌词源用于排版的纯斜杠分隔行。
fn is_content_text(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && !value
            .chars()
            .all(|character| matches!(character, '/' | '／'))
}

/// 按时间戳把翻译或音译行合并到原文，避免平台适配器复制双指针逻辑。
pub fn merge_auxiliary_lines(
    original: &mut [LyricLine],
    auxiliary: &[LyricLine],
    target: AuxiliaryKind,
) {
    let mut auxiliary_index = 0;
    for line in original {
        while auxiliary_index + 1 < auxiliary.len()
            && auxiliary[auxiliary_index + 1].start_ms <= line.start_ms
        {
            auxiliary_index += 1;
        }
        let candidates = [
            auxiliary.get(auxiliary_index),
            auxiliary.get(auxiliary_index + 1),
        ];
        let Some(matched) = candidates
            .into_iter()
            .flatten()
            .min_by_key(|candidate| candidate.start_ms.abs_diff(line.start_ms))
            .filter(|candidate| {
                candidate.start_ms.abs_diff(line.start_ms) <= AUXILIARY_TIME_TOLERANCE_MS
                    && is_content_text(&candidate.text)
            })
        else {
            continue;
        };
        match target {
            AuxiliaryKind::Translation => line.translation = Some(matched.text.clone()),
            AuxiliaryKind::Romanization => line.romanization = Some(matched.text.clone()),
        }
    }
}

/// 辅助歌词写入目标。
#[derive(Clone, Copy)]
pub enum AuxiliaryKind {
    Translation,
    Romanization,
}

fn convert_line(line: LineInfo) -> Option<LyricLine> {
    let start_ms = u64::try_from(line.start_time()?).ok()?;
    let end_ms = line
        .end_time()
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(start_ms);
    let text = line.text_from_any();
    if !is_content_text(&text) {
        return None;
    }
    let (words, translation, romanization) = match &line {
        LineInfo::Syllable { syllables, .. } => (convert_words(syllables), None, None),
        LineInfo::FullSyllable {
            syllables,
            translations,
            pronunciation,
            ..
        } => (
            convert_words(syllables),
            select_translation(translations),
            pronunciation.clone().filter(|value| is_content_text(value)),
        ),
        LineInfo::FullLine {
            translations,
            pronunciation,
            ..
        } => (
            Vec::new(),
            select_translation(translations),
            pronunciation.clone().filter(|value| is_content_text(value)),
        ),
        LineInfo::Line { .. } => (Vec::new(), None, None),
    };
    Some(LyricLine {
        start_ms,
        end_ms,
        text,
        translation,
        romanization,
        words,
    })
}

fn convert_words(syllables: &[SyllableInfo]) -> Vec<LyricWord> {
    syllables
        .iter()
        .filter_map(|syllable| {
            let start_ms = u64::try_from(syllable.start_time).ok()?;
            let end_ms = u64::try_from(syllable.end_time).ok()?;
            (!syllable.text.is_empty() && end_ms > start_ms).then(|| LyricWord {
                start_ms,
                end_ms,
                text: syllable.text.clone(),
            })
        })
        .collect()
}

fn select_translation(translations: &std::collections::HashMap<String, String>) -> Option<String> {
    translations
        .get("zh")
        .filter(|value| is_content_text(value))
        .or_else(|| translations.values().find(|value| is_content_text(value)))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::model::{PlatformNotice, platform_notice};

    /// QQ 音乐给纯音乐曲目返回的整段歌词不带时间戳，逐行解析会把它整段丢掉；补出的那一行
    /// 必须能让统一的占位识别判成纯音乐，否则这首歌既不显示歌词也判不出结论。
    #[test]
    fn untimed_platform_notice_survives_parsing() {
        let lines = parse_lrc_lines("此歌曲为没有填词的纯音乐，请您欣赏").expect("解析不应失败");
        assert_eq!(platform_notice(&lines), Some(PlatformNotice::Instrumental));
    }

    /// 没有时间戳的真实歌词同样解析不出行，不能因此被误判成占位文案。
    #[test]
    fn untimed_real_lyrics_are_still_dropped() {
        assert!(
            parse_lrc_lines("让我们一起摇摆")
                .expect("解析不应失败")
                .is_empty()
        );
    }
}
