use crate::lyrics::model::{
    LyricsPrecision, LyricsSnapshot, LyricsStatus, ResolvedLyrics, has_word_timing,
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
    if matches!(
        cached.status,
        LyricsStatus::Instrumental | LyricsStatus::NoLyrics
    ) {
        return !local.lines.is_empty();
    }
    has_word_timing(&local.lines)
        || auxiliary_content_quality(&local.lines) > auxiliary_content_quality(&cached.lines)
}
