//! 单个会话的元数据、播放状态与时间线刷新。
//!
//! 这里集中了“切歌瞬间”的全部判据：GSMTC 的标题与时间线是两条独立通道，
//! 标题换了而时间线还停在上一首时，必须先把上一首的大进度挂起，
//! 否则歌词层会拿到错误的曲目时长，把有词的歌判成没有歌词。

use std::time::Instant;

use crate::logging;
use crate::media::{
    MediaMetadata, model::MediaPlaybackStatus, players::selection_hold_after_title_change,
    thumbnail::read_thumbnail_data_url,
};

use super::{MetadataRefresh, SessionEntry, TimelineRefresh, session};

/// 管理器变化时补读所有轻量播放状态，兼容漏发单会话事件的播放器。
pub(super) fn refresh_all_playback(
    entries: &mut [SessionEntry],
    next_activity_order: &mut u64,
    selected_id: Option<u64>,
) -> bool {
    entries.iter_mut().fold(false, |selected_changed, entry| {
        let changed = refresh_playback_entry(entry, next_activity_order);
        selected_changed || (changed && Some(entry.id) == selected_id)
    })
}

/// 只刷新指定会话的歌曲元数据，避免播放事件重复读取和编码封面。
pub(super) fn refresh_metadata(entries: &mut [SessionEntry], session_id: u64) -> MetadataRefresh {
    let Some(entry) = entries.iter_mut().find(|entry| entry.id == session_id) else {
        return MetadataRefresh::default();
    };
    let Ok(properties) =
        session::read_properties(&entry.registration.session).inspect_err(|error| {
            logging::warn_throttled("media-properties-read", || {
                format!("刷新媒体属性失败: {error}")
            });
        })
    else {
        return MetadataRefresh::default();
    };
    // 文本内容不变且封面已经就绪时直接结束，跳过封面流的读取、解码与 Base64 编码。
    if entry.thumbnail_key.as_ref() == Some(&properties.text)
        && entry.snapshot.metadata.thumbnail_data_url.is_some()
    {
        return MetadataRefresh::default();
    }
    let thumbnail_data_url = properties
        .thumbnail
        .as_ref()
        .and_then(|thumbnail| read_thumbnail_data_url(thumbnail).ok().flatten());
    // 只有真正取到封面才记录内容键，使“封面晚于标题到达”的播放器能在下一次刷新补上。
    entry.thumbnail_key = thumbnail_data_url.as_ref().map(|_| properties.text.clone());

    let metadata = MediaMetadata {
        title: properties.text.title,
        artist: properties.text.artist,
        album_artist: properties.text.album_artist,
        subtitle: properties.text.subtitle,
        thumbnail_data_url,
    };
    if entry.snapshot.metadata == metadata {
        return MetadataRefresh::default();
    }
    let title_changed = entry.snapshot.metadata.title != metadata.title
        && !entry.snapshot.metadata.title.is_empty()
        && !metadata.title.is_empty();
    if title_changed {
        entry.selection_hold_until = selection_hold_after_title_change(entry.snapshot.player)
            .and_then(|duration| Instant::now().checked_add(duration));
    }
    entry.snapshot.metadata = metadata;
    MetadataRefresh {
        changed: true,
        track_boundary: title_changed,
    }
}

/// 该会话是否正处于“标题已换、时间线尚未确认新曲”的窗口。
///
/// 这个窗口里快照的时间线仍属于上一首，所以歌词层不会被通知
/// （见 `publisher::publish_selected_snapshot`）。
pub(super) fn timeline_pending_new_track(entries: &[SessionEntry], session_id: u64) -> bool {
    entries
        .iter()
        .find(|entry| entry.id == session_id)
        .is_some_and(|entry| entry.pending_previous_position_ms.is_some())
}

/// 标题已切换而时间线未出现新曲边界时，不向上层泄漏上一首的大进度。
pub(super) fn reset_stale_timeline_at_track_boundary(
    entries: &mut [SessionEntry],
    session_id: u64,
    metadata_track_boundary: bool,
    timeline_track_boundary: bool,
) {
    if !metadata_track_boundary {
        return;
    }
    let Some(entry) = entries.iter_mut().find(|entry| entry.id == session_id) else {
        return;
    };
    if timeline_track_boundary {
        entry.pending_previous_position_ms = None;
        return;
    }
    let Some(timeline) = entry.snapshot.timeline.as_mut() else {
        entry.pending_previous_position_ms = None;
        return;
    };
    if timeline.position_ms > timeline.start_time_ms.saturating_add(3_000) {
        entry.pending_previous_position_ms = Some(timeline.position_ms);
        timeline.position_ms = timeline.start_time_ms;
    } else {
        entry.pending_previous_position_ms = None;
    }
}

/// 刷新指定会话播放状态，并记录进入播放态的严格递增顺序。
pub(super) fn refresh_playback(
    entries: &mut [SessionEntry],
    session_id: u64,
    next_activity_order: &mut u64,
) -> bool {
    let Some(entry) = entries.iter_mut().find(|entry| entry.id == session_id) else {
        return false;
    };
    refresh_playback_entry(entry, next_activity_order)
}

/// 刷新单个会话的播放状态，忽略播放器重复推送的等值事件。
fn refresh_playback_entry(entry: &mut SessionEntry, next_activity_order: &mut u64) -> bool {
    let Ok(playback) = session::read_playback(&entry.registration.session).inspect_err(|error| {
        logging::warn_throttled("media-playback-read", || {
            format!("刷新媒体播放状态失败: {error}")
        });
    }) else {
        return false;
    };
    if entry.snapshot.playback == playback {
        return false;
    }
    if entry.snapshot.playback.status != MediaPlaybackStatus::Playing
        && playback.status == MediaPlaybackStatus::Playing
    {
        entry.activity_order = *next_activity_order;
        *next_activity_order = next_activity_order.saturating_add(1);
    }
    entry.snapshot.playback = playback;
    true
}

/// 刷新指定会话的时间线，并标记是否需要重新比较重复会话质量。
pub(super) fn refresh_timeline(entries: &mut [SessionEntry], session_id: u64) -> TimelineRefresh {
    let Some(entry) = entries.iter_mut().find(|entry| entry.id == session_id) else {
        return TimelineRefresh::default();
    };
    let timeline = session::read_timeline(&entry.registration.session).unwrap_or_else(|error| {
        logging::warn_throttled("media-timeline-read", || {
            format!("刷新媒体时间线失败: {error}")
        });
        None
    });
    if let Some(previous_position_ms) = entry.pending_previous_position_ms {
        let confirms_new_track = match (&entry.snapshot.timeline, &timeline) {
            (_, None) | (None, Some(_)) => true,
            (Some(current), Some(next)) => {
                next.start_time_ms != current.start_time_ms
                    || next.end_time_ms != current.end_time_ms
                    || next.position_ms <= next.start_time_ms.saturating_add(3_000)
                    || next.position_ms.saturating_add(5_000) < previous_position_ms
            }
        };
        if !confirms_new_track {
            return TimelineRefresh::default();
        }
        entry.pending_previous_position_ms = None;
    }
    if entry.snapshot.timeline == timeline {
        return TimelineRefresh::default();
    }
    let availability_changed = entry.snapshot.timeline.is_some() != timeline.is_some();
    let track_boundary = match (&entry.snapshot.timeline, &timeline) {
        (Some(previous), Some(next)) => {
            previous.start_time_ms != next.start_time_ms
                || previous.end_time_ms != next.end_time_ms
                || (next.position_ms <= next.start_time_ms.saturating_add(3_000)
                    && next.position_ms.saturating_add(5_000) < previous.position_ms)
        }
        _ => availability_changed,
    };
    entry.snapshot.timeline = timeline;
    TimelineRefresh {
        changed: true,
        availability_changed,
        track_boundary,
    }
}
