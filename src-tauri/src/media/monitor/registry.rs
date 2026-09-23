//! GSMTC 会话注册表的同步：新增会话建立订阅、失效会话被回收。

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
};

use crate::media::{MediaPlayer, model::MediaPlaybackStatus, players::identify};

use super::{SessionEntry, channel::WorkerSender, session};

/// 同步当前全部 GSMTC 会话，并为新增会话建立独立事件订阅。
pub(super) fn synchronize_sessions(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    sender: &WorkerSender,
    entries: &mut Vec<SessionEntry>,
    next_session_id: &mut u64,
    next_activity_order: &mut u64,
    new_playing_session_is_active: bool,
    only_supported_players: bool,
) {
    let Ok(view) = manager.GetSessions() else {
        return;
    };
    let sessions = (0..view.Size().unwrap_or_default())
        .filter_map(|index| view.GetAt(index).ok())
        .collect::<Vec<_>>();

    entries.retain(|entry| {
        sessions.contains(&entry.registration.session)
            && (!only_supported_players || entry.snapshot.player != MediaPlayer::Other)
    });

    for session in sessions {
        if entries
            .iter()
            .any(|entry| entry.registration.session == session)
        {
            continue;
        }
        if only_supported_players && !is_supported_session(&session) {
            continue;
        }

        let id = *next_session_id;
        *next_session_id = next_session_id.saturating_add(1);
        let Some(registration) = session::bind_session(session, id, sender) else {
            continue;
        };
        let Ok(snapshot) = session::read_snapshot(&registration.session) else {
            continue;
        };
        // 活动顺序的不变量：`0` 表示“还没有活动记录”，因此真实序号从 1 起分配；
        // 值越大表示越近进入播放态，择优算法直接把它当作“新近程度”比较（见 `selector`）。
        // 新建会话若此刻不在播放，就保留 0，让它排在有记录的会话之后。
        let activity_order = if new_playing_session_is_active
            && snapshot.playback.status == MediaPlaybackStatus::Playing
        {
            let order = *next_activity_order;
            *next_activity_order = next_activity_order.saturating_add(1);
            order
        } else {
            0
        };
        entries.push(SessionEntry {
            id,
            registration,
            thumbnail_key: snapshot
                .metadata
                .thumbnail_data_url
                .as_ref()
                .map(|_| session::MediaMetadataText::from_metadata(&snapshot.metadata)),
            snapshot,
            activity_order,
            selection_hold_until: None,
            pending_previous_position_ms: None,
        });
    }
}

/// 在建立事件订阅前判断会话是否属于四个已接入播放器。
fn is_supported_session(session: &GlobalSystemMediaTransportControlsSession) -> bool {
    session
        .SourceAppUserModelId()
        .map(|source_app_id| identify(&source_app_id.to_string()).player != MediaPlayer::Other)
        .unwrap_or(false)
}

/// 诊断输出里把空的元数据字段折叠成 `None`，便于前端区分“没有”与“空串”。
pub(super) fn non_empty_metadata(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_owned())
}
