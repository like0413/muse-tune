//! 歌词中文简繁转换；统一服务匹配字段、显示快照与解析后缓存。

use std::sync::LazyLock;

use opencc_fmmseg::{OpenCC, OpenccConfig};

use super::model::{LyricsChineseVariant, LyricsSnapshot};

static CONVERTER: LazyLock<OpenCC> = LazyLock::new(OpenCC::new);

/// 将歌曲匹配与缓存身份字段统一到简体字形；国内平台通常返回简体元数据。
pub(super) fn simplify_for_matching(text: &str) -> String {
    convert_text(text, OpenccConfig::T2s)
}

/// 消费已分配的匹配字段；原文已是简体或没有简繁差异时复用原字符串缓冲区。
pub(super) fn simplify_owned_for_matching(mut text: String) -> String {
    convert_string_if_needed(&mut text, OpenccConfig::T2s);
    text
}

/// 批量转换短文本，供歌曲名与歌手等媒体展示字段复用同一套 OpenCC 词典。
pub(super) fn convert_texts(mut texts: Vec<String>, variant: LyricsChineseVariant) -> Vec<String> {
    let Some(config) = conversion_config(variant) else {
        return texts;
    };
    for text in &mut texts {
        convert_string_if_needed(text, config);
    }
    texts
}

/// 按实际输出目标转换歌词正文、翻译与逐字片段，所有时间信息保持不变。
pub(super) fn convert_snapshot(
    snapshot: LyricsSnapshot,
    variant: LyricsChineseVariant,
) -> LyricsSnapshot {
    convert_snapshot_with_outcome(snapshot, variant).0
}

/// 转换歌词并返回正文是否真的发生变化，供缓存链路区分“转换”与“仅更新字形标记”。
pub(super) fn convert_snapshot_with_outcome(
    mut snapshot: LyricsSnapshot,
    variant: LyricsChineseVariant,
) -> (LyricsSnapshot, bool) {
    let Some(config) = conversion_config(variant) else {
        return (snapshot, false);
    };
    let mut converted = false;
    for line in &mut snapshot.lines {
        let line_converted = convert_string_if_needed(&mut line.text, config);
        converted |= line_converted;
        if let Some(translation) = &mut line.translation {
            converted |= convert_string_if_needed(translation, config);
        }
        // 逐字片段与正文来自同一行；正文无需转换时直接跳过整组检测，避免每个字都跑分类。
        if line_converted {
            for word in &mut line.words {
                convert_han_text(&mut word.text, config);
            }
        }
    }
    (snapshot, converted)
}

/// “简体/繁体”只改变字形，不启用地区词汇或标点替换。
const fn conversion_config(variant: LyricsChineseVariant) -> Option<OpenccConfig> {
    match variant {
        LyricsChineseVariant::Original => None,
        LyricsChineseVariant::Simplified => Some(OpenccConfig::T2s),
        LyricsChineseVariant::Traditional => Some(OpenccConfig::S2t),
    }
}

fn convert_text(text: &str, config: OpenccConfig) -> String {
    if needs_conversion(text, config) {
        CONVERTER.convert_with_config(text, config, false)
    } else {
        text.to_owned()
    }
}

/// 只有检测结果明确属于相反字形时才转换；无差异汉字对两种目标都可直接复用。
fn convert_string_if_needed(text: &mut String, config: OpenccConfig) -> bool {
    if needs_conversion(text, config) {
        *text = CONVERTER.convert_with_config(text, config, false);
        true
    } else {
        false
    }
}

/// 正文已确认属于相反字形后，逐字片段只需避开非汉字，不再重复执行字形检测。
fn convert_han_text(text: &mut String, config: OpenccConfig) {
    if text.chars().any(is_han_character) {
        *text = CONVERTER.convert_with_config(text, config, false);
    }
}

fn needs_conversion(text: &str, config: OpenccConfig) -> bool {
    if !text.chars().any(is_han_character) {
        return false;
    }
    matches!(
        (config, CONVERTER.zho_check(text)),
        (OpenccConfig::T2s, 1) | (OpenccConfig::S2t, 2)
    )
}

/// 非中文歌词不触发词典解压；转换器会在首段含汉字文本到达时再初始化。
const fn is_han_character(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2FA1F}'
            | '\u{30000}'..='\u{323AF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::model::{LyricLine, LyricWord};

    fn snapshot(text: &str) -> LyricsSnapshot {
        LyricsSnapshot {
            lines: vec![LyricLine {
                start_ms: 100,
                end_ms: 2_000,
                text: text.to_owned(),
                translation: Some(text.to_owned()),
                romanization: Some("ni hao".to_owned()),
                words: vec![LyricWord {
                    start_ms: 100,
                    end_ms: 500,
                    text: text.to_owned(),
                }],
            }],
            ..LyricsSnapshot::default()
        }
    }

    #[test]
    fn convert_snapshot_converts_traditional_text_without_changing_timing() {
        let converted = convert_snapshot(snapshot("音樂後臺"), LyricsChineseVariant::Simplified);

        assert_eq!(
            (
                converted.lines[0].text.as_str(),
                converted.lines[0].start_ms,
                converted.lines[0].words[0].text.as_str(),
            ),
            ("音乐后台", 100, "音乐后台")
        );
    }

    #[test]
    fn convert_snapshot_converts_simplified_text_without_touching_romanization() {
        let converted = convert_snapshot(snapshot("音乐后台"), LyricsChineseVariant::Traditional);

        assert_eq!(
            (
                converted.lines[0].text.as_str(),
                converted.lines[0].translation.as_deref(),
                converted.lines[0].romanization.as_deref(),
            ),
            ("音樂後臺", Some("音樂後臺"), Some("ni hao"))
        );
    }

    #[test]
    fn convert_snapshot_reuses_text_already_in_target_variant() {
        let source = snapshot("音樂後臺");
        let text_pointer = source.lines[0].text.as_ptr();

        let converted = convert_snapshot(source, LyricsChineseVariant::Traditional);

        assert_eq!(converted.lines[0].text.as_ptr(), text_pointer);
    }

    #[test]
    fn conversion_skips_characters_without_script_difference() {
        assert!(!needs_conversion("你好 hello", OpenccConfig::T2s));
        assert!(!needs_conversion("你好 hello", OpenccConfig::S2t));
    }
}
