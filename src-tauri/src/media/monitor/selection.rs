//! 会话择优：在多个播放器会话中选出该展示哪一个，并在目标变化时重绑下游资源。
//!
//! 择优不是简单的“最后播放的赢”：还要考虑系统当前会话、元数据完整度、以及
//! 切歌瞬间的短暂保持窗口（见 `selection_hold_until`）。

use std::time::Instant;

use tauri::Runtime;
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

use crate::error::Error;
use crate::media::{
    MediaMetadata, MediaSessionSelectionPolicy,
    activation::toggle_player_window,
    players::identify,
    selector::{SelectionCandidate, select_session},
    supported_players,
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
) -> Result<(), Error> {
    let entry = entries
        .iter()
        .find(|entry| Some(entry.id) == selected_id)
        .ok_or_else(|| Error::Message("当前没有可打开的媒体播放器".to_owned()))?;
    let source_app_id = entry
        .registration
        .session
        .SourceAppUserModelId()
        .map(|value| value.to_string())
        .map_err(|error| Error::Message(format!("读取当前播放器来源失败: {error}")))?;
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

/// 去重固定优先级并补齐全部已接入播放器，抵御损坏的持久化配置。
///
/// 补齐依据是共享配置里的规范平台顺序，新增平台只需登记那一处。
pub(super) fn normalize_selection_policy(
    mut policy: MediaSessionSelectionPolicy,
) -> MediaSessionSelectionPolicy {
    let supported = supported_players();
    policy
        .player_priority
        .retain(|player| supported.contains(player));
    let mut normalized = Vec::with_capacity(supported.len());
    for player in policy.player_priority {
        if !normalized.contains(&player) {
            normalized.push(player);
        }
    }
    for player in supported {
        if !normalized.contains(player) {
            normalized.push(*player);
        }
    }
    policy.player_priority = normalized;
    policy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::MediaPlayer;

    /// 占位默认策略必须能原样通过归一化：否则前端策略推送到达前后会选出不同的会话。
    #[test]
    fn default_policy_survives_normalization() {
        let default = MediaSessionSelectionPolicy::default();

        assert_eq!(
            normalize_selection_policy(default.clone()).player_priority,
            default.player_priority
        );
    }

    /// 缺失的已接入播放器必须补齐，否则该平台永远拿不到优先级。
    #[test]
    fn normalization_appends_missing_supported_players() {
        let policy = MediaSessionSelectionPolicy {
            player_priority: vec![MediaPlayer::KugouMusic],
            ..Default::default()
        };

        let normalized = normalize_selection_policy(policy);

        assert_eq!(
            normalized.player_priority,
            vec![
                MediaPlayer::KugouMusic,
                MediaPlayer::QqMusic,
                MediaPlayer::NeteaseCloudMusic,
                MediaPlayer::SodaMusic,
            ]
        );
    }

    /// 未接入的播放器必须剔除，否则“其他播放器”会占掉一个优先级名额。
    #[test]
    fn normalization_drops_unsupported_players() {
        let policy = MediaSessionSelectionPolicy {
            player_priority: vec![MediaPlayer::Other, MediaPlayer::QqMusic],
            ..Default::default()
        };

        let normalized = normalize_selection_policy(policy);

        assert!(!normalized.player_priority.contains(&MediaPlayer::Other));
        assert_eq!(normalized.player_priority.len(), 4);
    }

    /// 重复项只保留首次出现的位置，否则同一个播放器会在优先级列表里出现两次。
    #[test]
    fn normalization_keeps_first_occurrence_order() {
        let policy = MediaSessionSelectionPolicy {
            player_priority: vec![
                MediaPlayer::SodaMusic,
                MediaPlayer::QqMusic,
                MediaPlayer::SodaMusic,
            ],
            ..Default::default()
        };

        let normalized = normalize_selection_policy(policy);

        assert_eq!(
            normalized.player_priority,
            vec![
                MediaPlayer::SodaMusic,
                MediaPlayer::QqMusic,
                MediaPlayer::NeteaseCloudMusic,
                MediaPlayer::KugouMusic,
            ]
        );
    }

    /// 完整度按可用字段数量累计，供同曲目重复会话之间比较质量。
    #[test]
    fn metadata_completeness_counts_available_fields() {
        assert_eq!(metadata_completeness(&MediaMetadata::default()), 0);

        let full = MediaMetadata {
            title: "歌名".to_owned(),
            artist: "歌手".to_owned(),
            album_artist: "专辑歌手".to_owned(),
            subtitle: "副标题".to_owned(),
            thumbnail_data_url: Some("data:image/png;base64,".to_owned()),
        };

        assert_eq!(metadata_completeness(&full), 5);
    }
}
