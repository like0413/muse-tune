//! 会话择优：在多个播放器会话中选出该展示哪一个，并在目标变化时重绑下游资源。
//!
//! 择优不是简单的“最后播放的赢”：还要考虑系统当前会话、元数据完整度、以及
//! 切歌瞬间的短暂保持窗口（见 `selection_hold_until`）。

use std::time::Instant;

use tauri::Runtime;
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

use crate::media::{
    MediaMetadata, MediaPlayer, MediaSessionSelectionPolicy,
    activation::toggle_player_window,
    players::identify,
    selector::{SelectionCandidate, select_session},
};

use super::{
    SelectedMedia, SessionEntry,
    deadlines::WorkerDeadlines,
    publisher::{MediaSnapshotPublisher, publish_selected_snapshot, publish_volume},
    volume_binding::bind_selected_volume,
};

/// 按当前策略重新选择会话，仅在目标或已显示内容变化时广播。
pub(super) fn reconcile_selection<R: Runtime>(
    publisher: &MediaSnapshotPublisher<'_, R>,
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    entries: &[SessionEntry],
    selected_id: &mut Option<u64>,
    policy: &MediaSessionSelectionPolicy,
    force_publish: bool,
) -> bool {
    let windows_current = manager.GetCurrentSession().ok();
    let now = Instant::now();
    let candidates = entries
        .iter()
        .map(|entry| SelectionCandidate {
            id: entry.id,
            player: entry.snapshot.player,
            status: entry.snapshot.playback.status,
            activity_order: entry.activity_order,
            is_windows_current: windows_current
                .as_ref()
                .is_some_and(|session| entry.registration.session == *session),
            title: &entry.snapshot.metadata.title,
            artist: &entry.snapshot.metadata.artist,
            has_timeline: entry.snapshot.timeline.is_some(),
            metadata_completeness: metadata_completeness(&entry.snapshot.metadata),
            selection_held: entry
                .selection_hold_until
                .is_some_and(|deadline| now < deadline),
        })
        .collect::<Vec<_>>();
    let next_id = select_session(&candidates, *selected_id, policy);
    let selection_changed = next_id != *selected_id;
    *selected_id = next_id;

    if selection_changed || force_publish {
        publish_selected_snapshot(publisher, entries, next_id);
    }
    selection_changed
}

/// 统一处理会话选择与音量目标切换，避免各事件分支重复绑定逻辑。
pub(super) fn reconcile_selection_and_volume<R: Runtime>(
    publisher: &MediaSnapshotPublisher<'_, R>,
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    entries: &[SessionEntry],
    selected: &mut SelectedMedia<R>,
    deadlines: &mut WorkerDeadlines,
    policy: &MediaSessionSelectionPolicy,
    force_publish: bool,
) {
    if reconcile_selection(
        publisher,
        manager,
        entries,
        &mut selected.id,
        policy,
        force_publish,
    ) {
        bind_selected_volume(&mut selected.volume, deadlines, entries, selected.id);
        selected.spectrum.bind(selected.volume.capture_process_id());
        publish_volume(publisher.app, selected.volume.snapshot());
    }
}

/// 从当前选择读取稳定来源标识，并交由窗口开关模块处理。
pub(super) fn toggle_selected_player_window(
    entries: &[SessionEntry],
    selected_id: Option<u64>,
) -> Result<(), String> {
    let entry = entries
        .iter()
        .find(|entry| Some(entry.id) == selected_id)
        .ok_or_else(|| "当前没有可打开的媒体播放器".to_owned())?;
    let source_app_id = entry
        .registration
        .session
        .SourceAppUserModelId()
        .map(|value| value.to_string())
        .map_err(|error| format!("读取当前播放器来源失败: {error}"))?;
    let player = identify(&source_app_id);
    toggle_player_window(&source_app_id, &player, &entry.snapshot.metadata.title)
}

/// 计算用于同曲目重复会话择优的元数据完整度。
fn metadata_completeness(metadata: &MediaMetadata) -> u8 {
    [
        !metadata.title.is_empty(),
        !metadata.artist.is_empty(),
        !metadata.album_artist.is_empty(),
        !metadata.subtitle.is_empty(),
        metadata.thumbnail_data_url.is_some(),
    ]
    .into_iter()
    .map(u8::from)
    .sum()
}

/// 去重固定优先级并补齐四个已接入播放器，抵御损坏的持久化配置。
pub(super) fn normalize_selection_policy(
    mut policy: MediaSessionSelectionPolicy,
) -> MediaSessionSelectionPolicy {
    const SUPPORTED_PLAYERS: [MediaPlayer; 4] = [
        MediaPlayer::QqMusic,
        MediaPlayer::NeteaseCloudMusic,
        MediaPlayer::SodaMusic,
        MediaPlayer::KugouMusic,
    ];
    policy
        .player_priority
        .retain(|player| SUPPORTED_PLAYERS.contains(player));
    let mut normalized = Vec::with_capacity(SUPPORTED_PLAYERS.len());
    for player in policy.player_priority {
        if !normalized.contains(&player) {
            normalized.push(player);
        }
    }
    for player in SUPPORTED_PLAYERS {
        if !normalized.contains(&player) {
            normalized.push(player);
        }
    }
    policy.player_priority = normalized;
    policy
}
