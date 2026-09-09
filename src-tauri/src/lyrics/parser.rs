use std::panic::{AssertUnwindSafe, catch_unwind};

use lyrics_core::{LineInfo, LyricsData, SyllableInfo};
use lyrics_parsers::parsers::{krc_parser, lrc_parser, qrc_parser, yrc_parser};

use super::{
    error::LyricsError,
    model::{LyricLine, LyricWord},
};

const DEFAULT_LINE_DURATION_MS: u64 = 5_000;
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
    Ok(normalize_parsed_lines(parsed))
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
        lines[index].end_ms = lines
            .get(index + 1)
            .map_or(lines[index].start_ms + DEFAULT_LINE_DURATION_MS, |next| {
                next.start_ms.max(lines[index].start_ms + 1)
            });
    }
    lines
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
                    && !candidate.text.trim().is_empty()
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
    if text.trim().is_empty() {
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
            pronunciation
                .clone()
                .filter(|value| !value.trim().is_empty()),
        ),
        LineInfo::FullLine {
            translations,
            pronunciation,
            ..
        } => (
            Vec::new(),
            select_translation(translations),
            pronunciation
                .clone()
                .filter(|value| !value.trim().is_empty()),
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
            (!syllable.text.is_empty() && end_ms >= start_ms).then(|| LyricWord {
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
        .filter(|value| !value.trim().is_empty())
        .or_else(|| translations.values().find(|value| !value.trim().is_empty()))
        .cloned()
}
