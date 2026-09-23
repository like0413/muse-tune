//! 将平台返回的“纯音乐/暂无歌词”固定说明识别为语义状态。

use super::{
    model::{LyricLine, LyricsStatus},
    track::normalize_text,
};

const PLATFORM_NOTICE_MAX_CHARS: usize = 40;
const PLATFORM_NOTICE_MAX_LINES: usize = 3;
const NOTICE_SUBJECTS: [&str; 4] = ["此歌曲", "该歌曲", "本歌曲", "这首歌"];

/// 平台占位文案的类型；两者都不是歌词正文，但结论与有效期不同。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlatformNotice {
    Instrumental,
    NoLyrics,
}

impl PlatformNotice {
    /// 占位文案对应的歌曲状态。
    pub(super) const fn status(self) -> LyricsStatus {
        match self {
            Self::Instrumental => LyricsStatus::Instrumental,
            Self::NoLyrics => LyricsStatus::NoLyrics,
        }
    }
}

/// 识别平台给出的占位文案，并区分“纯音乐”与“没有歌词”。
pub(super) fn platform_notice(lines: &[LyricLine]) -> Option<PlatformNotice> {
    if lines.is_empty() || lines.len() > PLATFORM_NOTICE_MAX_LINES {
        return None;
    }
    lines.iter().find_map(|line| notice_kind(&line.text))
}

/// 从没有时间轴的原始歌词文本中找出平台占位文案。
pub(super) fn notice_text_without_timeline(input: &str) -> Option<String> {
    let texts = input
        .lines()
        .map(strip_bracket_segments)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    if texts.is_empty() || texts.len() > PLATFORM_NOTICE_MAX_LINES {
        return None;
    }
    texts.into_iter().find(|text| notice_kind(text).is_some())
}

/// 去掉方括号内的时间轴与元数据，只保留正文。
fn strip_bracket_segments(line: &str) -> String {
    let mut text = String::new();
    let mut inside_bracket = false;
    for character in line.chars() {
        match character {
            '[' => inside_bracket = true,
            ']' => inside_bracket = false,
            _ if !inside_bracket => text.push(character),
            _ => {}
        }
    }
    text.trim().to_owned()
}

/// 按归一化后的文案形态判断平台结论。
fn notice_kind(text: &str) -> Option<PlatformNotice> {
    let normalized = normalize_text(text);
    if normalized.is_empty() || normalized.chars().count() > PLATFORM_NOTICE_MAX_CHARS {
        return None;
    }
    if normalized.starts_with("纯音乐") {
        return Some(PlatformNotice::Instrumental);
    }
    if !NOTICE_SUBJECTS
        .iter()
        .any(|subject| normalized.starts_with(subject))
    {
        return None;
    }
    if normalized.contains("纯音乐") {
        return Some(PlatformNotice::Instrumental);
    }
    ["无歌词", "没有歌词", "暂无歌词"]
        .iter()
        .any(|marker| normalized.contains(marker))
        .then_some(PlatformNotice::NoLyrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lyrics::model::{LyricsSnapshot, LyricsSource, LyricsSourceKind, ResolvedLyrics},
        media::MediaPlayer,
    };

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

    #[test]
    fn recognizes_both_conclusions() {
        assert_eq!(
            platform_notice(&[line("纯音乐请欣赏")]),
            Some(PlatformNotice::Instrumental)
        );
        assert_eq!(
            platform_notice(&[line("此歌曲为没有填词的纯音乐，请您欣赏")]),
            Some(PlatformNotice::Instrumental)
        );
        assert_eq!(
            platform_notice(&[line("该歌曲暂无歌词")]),
            Some(PlatformNotice::NoLyrics)
        );
        assert_eq!(
            platform_notice(&[line("这首歌没有歌词")]),
            Some(PlatformNotice::NoLyrics)
        );
    }

    #[test]
    fn maps_to_matching_status() {
        assert_eq!(
            PlatformNotice::Instrumental.status(),
            LyricsStatus::Instrumental
        );
        assert_eq!(PlatformNotice::NoLyrics.status(), LyricsStatus::NoLyrics);
    }

    #[test]
    fn detection_requires_notice_shape_and_limits() {
        assert_eq!(platform_notice(&[line("我喜欢这首纯音乐作品")]), None);
        assert_eq!(platform_notice(&[line("此歌曲很好听")]), None);
        let long = format!("纯音乐{}", "啊".repeat(PLATFORM_NOTICE_MAX_CHARS));
        assert_eq!(platform_notice(&[line(&long)]), None);
        assert_eq!(
            platform_notice(&vec![line("纯音乐"); PLATFORM_NOTICE_MAX_LINES + 1]),
            None
        );
        assert_eq!(platform_notice(&[]), None);
    }

    #[test]
    fn detection_works_without_timeline() {
        assert_eq!(
            notice_text_without_timeline("此歌曲为没有填词的纯音乐，请您欣赏"),
            Some("此歌曲为没有填词的纯音乐，请您欣赏".to_owned())
        );
        assert_eq!(
            notice_text_without_timeline("[00:00.00]纯音乐，请欣赏"),
            Some("纯音乐，请欣赏".to_owned())
        );
        assert_eq!(notice_text_without_timeline("让我们一起摇摆"), None);
        assert_eq!(notice_text_without_timeline(""), None);
    }

    #[test]
    fn untimed_notice_becomes_instrumental_snapshot() {
        let lines = crate::lyrics::parser::parse_lrc_lines("此歌曲为没有填词的纯音乐，请您欣赏")
            .expect("解析不应失败");
        let snapshot = LyricsSnapshot::from_resolved(
            "track-key".to_owned(),
            ResolvedLyrics {
                source: LyricsSource {
                    player: MediaPlayer::QqMusic,
                    kind: LyricsSourceKind::Online,
                    song_id: None,
                },
                lines,
            },
        );
        assert_eq!(snapshot.status, LyricsStatus::Instrumental);
        assert!(snapshot.lines.is_empty());
    }
}
