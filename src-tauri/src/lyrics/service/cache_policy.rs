use crate::lyrics::{
    model::{LyricsPrecision, LyricsSnapshot, LyricsStatus, ResolvedLyrics, has_word_timing},
    notice::platform_notice,
};

use super::pipeline::auxiliary_content_quality;

/// 有效缓存是否还需要检查播放器本地内容升级。
///
/// - 逐行：本地可能给出逐字，是唯一的精度提升路径。
/// - 纯音乐与“没有歌词”：平台有时会把有歌词的歌标成这两种结论，而本地歌词的写入往往晚于
///   在线结果，一旦本地出现歌词就应该推翻它；两者的有效期都不短（30 天 / 7 天），
///   不能等它自然过期。
/// - 逐字不做检查：已是最精密结果。
pub(super) fn should_check_local_upgrade(snapshot: &LyricsSnapshot) -> bool {
    snapshot.precision == Some(LyricsPrecision::Line)
        || matches!(
            snapshot.status,
            LyricsStatus::Instrumental | LyricsStatus::NoLyrics
        )
}

/// 本地结果只有提高时间精度或辅助内容完整度时才替换已展示缓存。
pub(super) fn local_result_is_upgrade(cached: &LyricsSnapshot, local: &ResolvedLyrics) -> bool {
    // 纯音乐与“没有歌词”缓存都不携带歌词行，本地只要产出可展示歌词就构成升级：这是推翻误判的
    // 唯一机会，所以不能拿辅助内容覆盖率去比较（空行集的覆盖率恒为 0，反而会把升级挡掉）。
    // 但本地内容本身就是平台占位文案时（播放器把“此歌曲为没有填词的纯音乐”原样写进本地歌词
    // 文件）同样没有歌词行，不算升级，否则每次命中缓存都会“升级”成同一个结论。
    if matches!(
        cached.status,
        LyricsStatus::Instrumental | LyricsStatus::NoLyrics
    ) {
        return !local.lines.is_empty() && platform_notice(&local.lines).is_none();
    }
    has_word_timing(&local.lines)
        || auxiliary_content_quality(&local.lines) > auxiliary_content_quality(&cached.lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::model::{LyricLine, LyricWord, LyricsSource, LyricsSourceKind};
    use crate::media::MediaPlayer;

    fn line(start_ms: u64, end_ms: u64, text: &str) -> LyricLine {
        LyricLine {
            start_ms,
            end_ms,
            text: text.to_owned(),
            translation: None,
            romanization: None,
            words: Vec::new(),
        }
    }

    fn word_line(start_ms: u64, end_ms: u64, text: &str) -> LyricLine {
        LyricLine {
            words: vec![LyricWord {
                start_ms,
                end_ms,
                text: text.to_owned(),
            }],
            ..line(start_ms, end_ms, text)
        }
    }

    fn snapshot(status: LyricsStatus, precision: Option<LyricsPrecision>) -> LyricsSnapshot {
        LyricsSnapshot {
            status,
            precision,
            ..LyricsSnapshot::default()
        }
    }

    fn local(lines: Vec<LyricLine>) -> ResolvedLyrics {
        ResolvedLyrics {
            source: LyricsSource {
                player: MediaPlayer::QqMusic,
                kind: LyricsSourceKind::Local,
                song_id: None,
            },
            lines,
        }
    }

    /// 逐字缓存已是最精密结果，再查本地不可能有提升；纯音乐与"没有歌词"则相反，
    /// 平台会误判且本地歌词常常写入更晚，必须保留推翻机会。
    #[test]
    fn local_upgrade_check_targets_only_improvable_states() {
        assert!(should_check_local_upgrade(&snapshot(
            LyricsStatus::Ready,
            Some(LyricsPrecision::Line)
        )));
        assert!(should_check_local_upgrade(&snapshot(
            LyricsStatus::Instrumental,
            None
        )));
        assert!(should_check_local_upgrade(&snapshot(
            LyricsStatus::NoLyrics,
            None
        )));

        assert!(!should_check_local_upgrade(&snapshot(
            LyricsStatus::Ready,
            Some(LyricsPrecision::Word)
        )));
        assert!(!should_check_local_upgrade(&snapshot(
            LyricsStatus::Loading,
            None
        )));
        assert!(!should_check_local_upgrade(&snapshot(
            LyricsStatus::Unavailable,
            None
        )));
    }

    /// 语义结论不携带歌词行，本地只要产出可展示歌词就是升级——这是推翻误判的唯一机会；
    /// 但本地自己也是平台占位文案时并没有歌词行，不能当成升级。
    #[test]
    fn local_lyrics_override_semantic_conclusions() {
        let instrumental = snapshot(LyricsStatus::Instrumental, None);
        assert!(local_result_is_upgrade(
            &instrumental,
            &local(vec![line(0, 1_000, "第一句")])
        ));
        assert!(!local_result_is_upgrade(&instrumental, &local(Vec::new())));
        assert!(!local_result_is_upgrade(
            &instrumental,
            &local(vec![line(0, 1_000, "此歌曲为没有填词的纯音乐，请您欣赏")])
        ));
    }

    /// 已展示逐行时，本地逐字构成升级。
    #[test]
    fn word_timing_counts_as_local_upgrade() {
        let lined = snapshot(LyricsStatus::Ready, Some(LyricsPrecision::Line));
        assert!(local_result_is_upgrade(
            &lined,
            &local(vec![word_line(0, 1_000, "第一句")])
        ));
    }

    /// 本地逐行且辅助内容不更完整时不得替换，否则用户会看到歌词无意义地跳一次。
    #[test]
    fn equal_local_precision_is_not_an_upgrade() {
        let lined = LyricsSnapshot {
            lines: vec![line(0, 1_000, "第一句")],
            ..snapshot(LyricsStatus::Ready, Some(LyricsPrecision::Line))
        };
        assert!(!local_result_is_upgrade(
            &lined,
            &local(vec![line(0, 1_000, "别的歌词")])
        ));
    }

    /// 辅助内容覆盖率更高时视为升级。
    #[test]
    fn better_auxiliary_coverage_is_an_upgrade() {
        let lined = LyricsSnapshot {
            lines: vec![line(0, 1_000, "第一句")],
            ..snapshot(LyricsStatus::Ready, Some(LyricsPrecision::Line))
        };
        let richer = local(vec![LyricLine {
            translation: Some("first".to_owned()),
            ..line(0, 1_000, "第一句")
        }]);
        assert!(local_result_is_upgrade(&lined, &richer));
    }
}
