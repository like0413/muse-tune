//! 缓存条目的时效策略：可信期、在线复核与可缓存状态。
//!
//! 期限按结论的稳定性分级——越确定的活得越久。这里只做纯判定，
//! 不读写磁盘，因此可以脱离文件系统单独验证。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::lyrics::model::{LyricsPrecision, LyricsStatus};

use super::entry::CacheEntry;

/// 展示可信期：缓存可直接展示、无需阻塞式重新解析的期限。
///
/// 期限按结论的稳定性定：越确定的活得越久。逐字与纯音乐 30 天——逐字已是最精密的结果，
/// 纯音乐是歌曲自身的属性；逐行与“没有歌词”7 天——前者仍可能被升级成逐字，后者可能被
/// 之后补上的本地或在线歌词推翻。
const WORD_REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const LINE_REFRESH_INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const INSTRUMENTAL_REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const NO_LYRICS_REFRESH_INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub(super) fn is_fresh(entry: &CacheEntry) -> bool {
    is_fresh_at(entry, now_seconds())
}

pub(super) fn is_fresh_at(entry: &CacheEntry, current_seconds: u64) -> bool {
    let Some(age) = current_seconds.checked_sub(entry.refreshed_at_seconds) else {
        return false;
    };
    let Some(interval) = refresh_interval(entry) else {
        return false;
    };
    age < interval.as_secs()
}

pub(super) fn refresh_interval(entry: &CacheEntry) -> Option<Duration> {
    match entry.snapshot.status {
        LyricsStatus::Ready if entry.snapshot.precision == Some(LyricsPrecision::Word) => {
            Some(WORD_REFRESH_INTERVAL)
        }
        LyricsStatus::Ready => Some(LINE_REFRESH_INTERVAL),
        LyricsStatus::Instrumental => Some(INSTRUMENTAL_REFRESH_INTERVAL),
        LyricsStatus::NoLyrics => Some(NO_LYRICS_REFRESH_INTERVAL),
        LyricsStatus::Loading | LyricsStatus::Unavailable | LyricsStatus::Error => None,
    }
}

/// 是否需要在线复核。
///
/// 只有逐行需要：它仍可能被升级成逐字，且升级只能靠在线来源，所以每次播放都静默确认一次。
/// 逐字已是最精密的结论，复核不可能带来提升；纯音乐与无歌词只靠本地歌词复核（不联网），
/// 那一步由 `should_check_local_upgrade` 决定，不经过这里。
pub(super) fn needs_revalidation(entry: &CacheEntry) -> bool {
    entry.snapshot.status == LyricsStatus::Ready
        && entry.snapshot.precision == Some(LyricsPrecision::Line)
}

/// 只允许可稳定复用的解析结论进入磁盘缓存。
pub(super) fn is_cacheable_status(status: LyricsStatus) -> bool {
    matches!(
        status,
        LyricsStatus::Ready | LyricsStatus::Instrumental | LyricsStatus::NoLyrics
    )
}

pub(super) fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::model::LyricsSnapshot;

    /// 构造只关心结论与时间戳的缓存条目。
    fn entry(
        refreshed_at_seconds: u64,
        status: LyricsStatus,
        precision: Option<LyricsPrecision>,
    ) -> CacheEntry {
        CacheEntry {
            refreshed_at_seconds,
            chinese_variant: crate::lyrics::model::LyricsChineseVariant::Original,
            snapshot: LyricsSnapshot {
                status,
                precision,
                ..LyricsSnapshot::default()
            },
        }
    }

    /// 可信期按结论的稳定性分级：越确定的活得越久，否则会被反复无效重解析。
    #[test]
    fn refresh_intervals_follow_conclusion_stability() {
        assert_eq!(
            refresh_interval(&entry(0, LyricsStatus::Ready, Some(LyricsPrecision::Word))),
            Some(WORD_REFRESH_INTERVAL)
        );
        assert_eq!(
            refresh_interval(&entry(0, LyricsStatus::Ready, Some(LyricsPrecision::Line))),
            Some(LINE_REFRESH_INTERVAL)
        );
        assert_eq!(
            refresh_interval(&entry(0, LyricsStatus::Instrumental, None)),
            Some(INSTRUMENTAL_REFRESH_INTERVAL)
        );
        assert_eq!(
            refresh_interval(&entry(0, LyricsStatus::NoLyrics, None)),
            Some(NO_LYRICS_REFRESH_INTERVAL)
        );

        // 未完成的结论没有可信期。
        for status in [
            LyricsStatus::Loading,
            LyricsStatus::Unavailable,
            LyricsStatus::Error,
        ] {
            assert_eq!(refresh_interval(&entry(0, status, None)), None);
        }
    }

    /// 新鲜度是"年龄 < 可信期"，边界处必须恰好过期，否则会多展示一天。
    #[test]
    fn freshness_is_relative_to_stored_timestamp() {
        let line = entry(1_000, LyricsStatus::Ready, Some(LyricsPrecision::Line));
        let interval = LINE_REFRESH_INTERVAL.as_secs();

        assert!(is_fresh_at(&line, 1_000 + interval - 1));
        assert!(!is_fresh_at(&line, 1_000 + interval));
    }

    /// 系统时间回拨时 `checked_sub` 会失败，此时不能把条目当成刚写入的新鲜缓存。
    #[test]
    fn clock_going_backwards_is_not_fresh() {
        let line = entry(1_000, LyricsStatus::Ready, Some(LyricsPrecision::Line));
        assert!(!is_fresh_at(&line, 999));
    }

    #[test]
    fn non_cacheable_states_are_never_fresh() {
        for status in [
            LyricsStatus::Loading,
            LyricsStatus::Unavailable,
            LyricsStatus::Error,
        ] {
            assert!(!is_fresh_at(&entry(1_000, status, None), 1_000));
        }
    }

    /// 只有逐行需要在线复核：逐字已最精密，纯音乐与无歌词只走本地复核路径。
    #[test]
    fn needs_revalidation_only_for_line_precision() {
        assert!(needs_revalidation(&entry(
            0,
            LyricsStatus::Ready,
            Some(LyricsPrecision::Line)
        )));
        assert!(!needs_revalidation(&entry(
            0,
            LyricsStatus::Ready,
            Some(LyricsPrecision::Word)
        )));
        assert!(!needs_revalidation(&entry(
            0,
            LyricsStatus::Instrumental,
            None
        )));
        assert!(!needs_revalidation(&entry(0, LyricsStatus::NoLyrics, None)));
    }

    /// 只有可稳定复用的结论能落盘，中途状态写进缓存会让下次启动展示过期过程。
    #[test]
    fn only_stable_conclusions_are_cacheable() {
        for status in [
            LyricsStatus::Ready,
            LyricsStatus::Instrumental,
            LyricsStatus::NoLyrics,
        ] {
            assert!(is_cacheable_status(status), "{status:?} 应当可缓存");
        }
        for status in [
            LyricsStatus::Loading,
            LyricsStatus::Unavailable,
            LyricsStatus::Error,
        ] {
            assert!(!is_cacheable_status(status), "{status:?} 不应可缓存");
        }
    }
}
