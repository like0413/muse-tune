use crate::lyrics::model::{LyricsPrecision, LyricsSnapshot, ResolvedLyrics, has_word_timing};

use super::pipeline::auxiliary_content_quality;

/// 有效缓存是否还需要检查播放器本地内容升级。
pub(super) fn should_check_local_upgrade(snapshot: &LyricsSnapshot) -> bool {
    snapshot.precision != Some(LyricsPrecision::Word)
}

/// 本地结果只有提高时间精度或辅助内容完整度时才替换已展示缓存。
pub(super) fn local_result_is_upgrade(cached: &LyricsSnapshot, local: &ResolvedLyrics) -> bool {
    has_word_timing(&local.lines)
        || auxiliary_content_quality(&local.lines) > auxiliary_content_quality(&cached.lines)
}
