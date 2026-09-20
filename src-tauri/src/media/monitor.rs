use std::{
    sync::{Arc, RwLock, mpsc, mpsc::RecvTimeoutError},
    time::Instant,
};

use tauri::{AppHandle, Runtime};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
};

use super::{
    MediaControlAction, MediaMetadata, MediaPlayer, MediaSessionSelectionPolicy,
    MediaSessionSnapshot, MediaSnapshotSubscriber, MediaVolumeSnapshot,
    activation::toggle_player_window,
    model::MediaPlaybackStatus,
    players::{identify, selection_hold_after_title_change},
    selector::{SelectionCandidate, select_session},
    spectrum::AudioSpectrumController,
    thumbnail::read_thumbnail_data_url,
    volume::ApplicationVolumeController,
};

mod apartment;
mod deadlines;
pub(super) mod metrics;
pub(in crate::media) mod pending_events;
mod publisher;
mod service;
mod session;

use apartment::WinRtMta;
use deadlines::{ScheduledWorkerTask, WorkerDeadlines};
use metrics::{WorkerMessageKind, WorkerMetrics, WorkerReceiver, WorkerSender};
use publisher::{
    MediaSnapshotPublisher, publish_selected_snapshot, publish_selected_timeline, publish_volume,
};
pub use service::MediaService;
use session::SessionRegistration;

pub(super) enum WorkerMessage {
    EventsReady,
    SelectionPolicyChanged(
        MediaSessionSelectionPolicy,
        mpsc::SyncSender<Result<(), String>>,
    ),
    Control(MediaControlAction, mpsc::SyncSender<Result<bool, String>>),
    TogglePlayerWindow(mpsc::SyncSender<Result<(), String>>),
    GetVolume(mpsc::SyncSender<Option<MediaVolumeSnapshot>>),
    SetVolume(f32, mpsc::SyncSender<Result<MediaVolumeSnapshot, String>>),
    ToggleMute(mpsc::SyncSender<Result<MediaVolumeSnapshot, String>>),
    GetDiagnostics(mpsc::SyncSender<super::MediaRuntimeDiagnostics>),
    SpectrumEnabled(bool, u16, mpsc::SyncSender<Result<(), String>>),
    Shutdown,
}

/// 单个 GSMTC 会话的事件注册、快照与最近播放序号。
struct SessionEntry {
    id: u64,
    registration: SessionRegistration,
    snapshot: MediaSessionSnapshot,
    /// 当前封面所对应的文本元数据键；用于跳过未变内容的封面重复解码。
    thumbnail_key: Option<session::MediaMetadataText>,
    activity_order: u64,
    selection_hold_until: Option<Instant>,
    pending_previous_position_ms: Option<i64>,
}

/// 把当前媒体目标及其应用音量绑定保持为同一份运行时状态。
struct SelectedMedia<R: Runtime> {
    id: Option<u64>,
    volume: ApplicationVolumeController,
    spectrum: AudioSpectrumController<R>,
}

/// 单次时间线刷新结果，用于区分轻量位置更新与会话质量变化。
#[derive(Clone, Copy, Default)]
struct TimelineRefresh {
    changed: bool,
    availability_changed: bool,
    track_boundary: bool,
}

/// 单次媒体属性刷新结果，标题变化是比时间线更可靠的切歌信号。
#[derive(Clone, Copy, Default)]
struct MetadataRefresh {
    changed: bool,
    track_boundary: bool,
}

/// 初始化并运行串行事件循环；轮询不参与媒体状态同步。
fn run_worker<R: Runtime>(
    app: AppHandle<R>,
    sender: WorkerSender,
    receiver: WorkerReceiver,
    mut worker_metrics: WorkerMetrics,
    snapshot: Arc<RwLock<Option<MediaSessionSnapshot>>>,
    snapshot_subscriber: MediaSnapshotSubscriber,
) {
    let _apartment = match WinRtMta::initialize() {
        Ok(apartment) => apartment,
        Err(error) => {
            log::error!("初始化媒体会话 WinRT 线程失败: {error}");
            return;
        }
    };

    let manager = match session::register_manager(&sender) {
        Ok(manager) => manager,
        Err(error) => {
            log::error!("连接 Windows 媒体会话管理器失败: {error}");
            return;
        }
    };
    let mut sessions = Vec::new();
    let mut next_session_id = 1;
    let mut next_activity_order = 1;
    let mut selection_policy = MediaSessionSelectionPolicy::default();
    let mut selected = SelectedMedia {
        id: None,
        volume: ApplicationVolumeController::new(sender.clone()),
        spectrum: AudioSpectrumController::new(app.clone()),
    };
    let mut deadlines = WorkerDeadlines::default();
    let snapshot_publisher = MediaSnapshotPublisher {
        app: &app,
        snapshot: &snapshot,
        subscriber: &snapshot_subscriber,
    };
    synchronize_sessions(
        &manager.manager,
        &sender,
        &mut sessions,
        &mut next_session_id,
        &mut next_activity_order,
        false,
        selection_policy.only_supported_players,
    );
    reconcile_selection_and_volume(
        &snapshot_publisher,
        &manager.manager,
        &sessions,
        &mut selected,
        &mut deadlines,
        &selection_policy,
        true,
    );

    loop {
        for task in deadlines.take_due(Instant::now()) {
            match task {
                ScheduledWorkerTask::MetadataSettle(session_id) => {
                    handle_media_properties_change(
                        &snapshot_publisher,
                        &manager.manager,
                        &mut sessions,
                        &mut selected,
                        &mut deadlines,
                        &selection_policy,
                        session_id,
                    );
                }
                ScheduledWorkerTask::VolumeRebind { target_id, attempt } => {
                    handle_volume_rebind_due(
                        &app,
                        &sessions,
                        &mut selected,
                        &mut deadlines,
                        target_id,
                        attempt,
                    );
                }
            }
        }
        let envelope = if let Some(timeout) = deadlines.next_timeout(Instant::now()) {
            match receiver.recv_timeout(timeout) {
                Ok(envelope) => envelope,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            let Ok(envelope) = receiver.recv() else {
                break;
            };
            envelope
        };
        worker_metrics.record_received(&envelope);
        let message = envelope.into_message();
        match message {
            WorkerMessage::EventsReady => {
                let events = sender.take_pending_events();
                if events.manager_changed {
                    worker_metrics.record_event_processed(WorkerMessageKind::ManagerChanged);
                    synchronize_sessions(
                        &manager.manager,
                        &sender,
                        &mut sessions,
                        &mut next_session_id,
                        &mut next_activity_order,
                        true,
                        selection_policy.only_supported_players,
                    );
                    let selected_was_refreshed =
                        refresh_all_playback(&mut sessions, &mut next_activity_order, selected.id);
                    reconcile_selection_and_volume(
                        &snapshot_publisher,
                        &manager.manager,
                        &sessions,
                        &mut selected,
                        &mut deadlines,
                        &selection_policy,
                        selected_was_refreshed,
                    );
                }
                for (session_id, observed_at) in events.media_properties_changed {
                    worker_metrics
                        .record_event_processed(WorkerMessageKind::MediaPropertiesChanged);
                    if deadlines.schedule_metadata_settle(session_id, observed_at) {
                        worker_metrics.record_coalesced_event();
                    }
                    worker_metrics
                        .observe_metadata_settle_pending(deadlines.metadata_settle_pending_count());
                    handle_media_properties_change(
                        &snapshot_publisher,
                        &manager.manager,
                        &mut sessions,
                        &mut selected,
                        &mut deadlines,
                        &selection_policy,
                        session_id,
                    );
                }
                for session_id in events.playback_info_changed {
                    worker_metrics.record_event_processed(WorkerMessageKind::PlaybackInfoChanged);
                    let playback_changed =
                        refresh_playback(&mut sessions, session_id, &mut next_activity_order);
                    let metadata_refresh = if playback_changed {
                        refresh_metadata(&mut sessions, session_id)
                    } else {
                        MetadataRefresh::default()
                    };
                    let pending_before = timeline_pending_new_track(&sessions, session_id);
                    let timeline_refresh = refresh_timeline(&mut sessions, session_id);
                    reset_stale_timeline_at_track_boundary(
                        &mut sessions,
                        session_id,
                        metadata_refresh.track_boundary,
                        timeline_refresh.track_boundary,
                    );
                    let track_confirmed =
                        pending_before && !timeline_pending_new_track(&sessions, session_id);
                    if playback_changed
                        || metadata_refresh.changed
                        || timeline_refresh.availability_changed
                        || track_confirmed
                    {
                        let selected_was_refreshed = selected.id == Some(session_id);
                        reconcile_selection_and_volume(
                            &snapshot_publisher,
                            &manager.manager,
                            &sessions,
                            &mut selected,
                            &mut deadlines,
                            &selection_policy,
                            selected_was_refreshed,
                        );
                    } else if timeline_refresh.changed && selected.id == Some(session_id) {
                        publish_selected_timeline(&app, &snapshot, &sessions, selected.id);
                    }
                }
                for session_id in events.timeline_properties_changed {
                    worker_metrics
                        .record_event_processed(WorkerMessageKind::TimelinePropertiesChanged);
                    let pending_before = timeline_pending_new_track(&sessions, session_id);
                    let timeline_refresh = refresh_timeline(&mut sessions, session_id);
                    let metadata_refresh = if timeline_refresh.track_boundary {
                        refresh_metadata(&mut sessions, session_id)
                    } else {
                        MetadataRefresh::default()
                    };
                    reset_stale_timeline_at_track_boundary(
                        &mut sessions,
                        session_id,
                        metadata_refresh.track_boundary,
                        timeline_refresh.track_boundary,
                    );
                    // 时间线刚确认新曲：必须重新发布一次完整快照，否则歌词层拿不到此前被推迟的通知。
                    let track_confirmed =
                        pending_before && !timeline_pending_new_track(&sessions, session_id);
                    if metadata_refresh.changed
                        || timeline_refresh.availability_changed
                        || timeline_refresh.track_boundary
                        || track_confirmed
                    {
                        let selected_was_refreshed = selected.id == Some(session_id);
                        reconcile_selection_and_volume(
                            &snapshot_publisher,
                            &manager.manager,
                            &sessions,
                            &mut selected,
                            &mut deadlines,
                            &selection_policy,
                            selected_was_refreshed,
                        );
                    } else if timeline_refresh.changed && selected.id == Some(session_id) {
                        publish_selected_timeline(&app, &snapshot, &sessions, selected.id);
                    }
                }
                for target_id in events.volume_sessions_changed {
                    worker_metrics.record_event_processed(WorkerMessageKind::VolumeSessionsChanged);
                    if selected.id == Some(target_id) {
                        rebind_selected_volume(&mut selected.volume, &sessions, target_id);
                        if selected.volume.snapshot().is_some() {
                            deadlines.cancel_volume_rebind();
                        }
                        selected.spectrum.bind(selected.volume.capture_process_id());
                        publish_volume(&app, selected.volume.snapshot());
                    }
                }
                for target_id in events.volume_changed {
                    worker_metrics.record_event_processed(WorkerMessageKind::VolumeChanged);
                    if selected.id == Some(target_id) {
                        if selected.volume.snapshot().is_some() {
                            deadlines.cancel_volume_rebind();
                        }
                        selected.spectrum.bind(selected.volume.capture_process_id());
                        publish_volume(&app, selected.volume.snapshot());
                    }
                }
            }
            WorkerMessage::SelectionPolicyChanged(policy, result_sender) => {
                let normalized = normalize_selection_policy(policy);
                if selection_policy != normalized {
                    selection_policy = normalized;
                    synchronize_sessions(
                        &manager.manager,
                        &sender,
                        &mut sessions,
                        &mut next_session_id,
                        &mut next_activity_order,
                        false,
                        selection_policy.only_supported_players,
                    );
                    reconcile_selection_and_volume(
                        &snapshot_publisher,
                        &manager.manager,
                        &sessions,
                        &mut selected,
                        &mut deadlines,
                        &selection_policy,
                        true,
                    );
                }
                let _ = result_sender.send(Ok(()));
            }
            WorkerMessage::Control(action, result_sender) => {
                let result = session::control(
                    sessions
                        .iter()
                        .find(|entry| Some(entry.id) == selected.id)
                        .map(|entry| &entry.registration),
                    action,
                );
                let _ = result_sender.send(result);
            }
            WorkerMessage::TogglePlayerWindow(result_sender) => {
                let result = toggle_selected_player_window(&sessions, selected.id);
                let _ = result_sender.send(result);
            }
            WorkerMessage::GetVolume(result_sender) => {
                // 首次订阅可能晚于播放器启动；读取前补齐尚未就绪的音频绑定。
                if selected.volume.snapshot().is_none()
                    && let Some(target_id) = selected.id
                {
                    rebind_selected_volume(&mut selected.volume, &sessions, target_id);
                    if selected.volume.snapshot().is_some() {
                        deadlines.cancel_volume_rebind();
                    }
                    selected.spectrum.bind(selected.volume.capture_process_id());
                    publish_volume(&app, selected.volume.snapshot());
                }
                let _ = result_sender.send(selected.volume.snapshot());
            }
            WorkerMessage::SetVolume(level, result_sender) => {
                let result = selected.volume.set_level(level);
                if let Ok(next) = result {
                    publish_volume(&app, Some(next));
                }
                let _ = result_sender.send(result);
            }
            WorkerMessage::ToggleMute(result_sender) => {
                let result = selected.volume.toggle_muted();
                if let Ok(next) = result {
                    publish_volume(&app, Some(next));
                }
                let _ = result_sender.send(result);
            }
            WorkerMessage::GetDiagnostics(result_sender) => {
                let (spectrum_enabled, spectrum_active) = selected.spectrum.diagnostics();
                let _ = result_sender.send(super::MediaRuntimeDiagnostics {
                    session_count: sessions.len(),
                    sessions: sessions
                        .iter()
                        .map(|entry| super::MediaRuntimeSessionDiagnostics {
                            player: entry.snapshot.player,
                            playback_status: entry.snapshot.playback.status,
                            title: non_empty_metadata(&entry.snapshot.metadata.title),
                            artist: non_empty_metadata(&entry.snapshot.metadata.artist),
                            timeline_available: entry.snapshot.timeline.is_some(),
                            selected: selected.id == Some(entry.id),
                        })
                        .collect(),
                    selection_strategy: selection_policy.strategy,
                    volume: selected.volume.snapshot(),
                    audio_process_id: selected.volume.capture_process_id(),
                    spectrum_enabled,
                    spectrum_active,
                    worker: worker_metrics.snapshot(deadlines.metadata_settle_pending_count()),
                });
            }
            WorkerMessage::SpectrumEnabled(enabled, frame_rate, result_sender) => {
                let result = selected.spectrum.set_enabled(enabled, frame_rate);
                let _ = result_sender.send(result);
            }
            WorkerMessage::Shutdown => break,
        }
    }

    drop(sessions);
    drop(manager);
}

/// 刷新单会话元数据与时间线，并保持选择、音量目标和发布结果一致。
fn handle_media_properties_change<R: Runtime>(
    publisher: &MediaSnapshotPublisher<'_, R>,
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    sessions: &mut [SessionEntry],
    selected: &mut SelectedMedia<R>,
    deadlines: &mut WorkerDeadlines,
    selection_policy: &MediaSessionSelectionPolicy,
    session_id: u64,
) {
    let pending_before = timeline_pending_new_track(sessions, session_id);
    let metadata_refresh = refresh_metadata(sessions, session_id);
    let timeline_refresh = refresh_timeline(sessions, session_id);
    reset_stale_timeline_at_track_boundary(
        sessions,
        session_id,
        metadata_refresh.track_boundary,
        timeline_refresh.track_boundary,
    );
    // 时间线刚确认新曲：必须重新发布一次完整快照，否则歌词层拿不到此前被推迟的通知。
    let track_confirmed = pending_before && !timeline_pending_new_track(sessions, session_id);
    if metadata_refresh.changed || timeline_refresh.availability_changed || track_confirmed {
        let selected_was_refreshed = selected.id == Some(session_id);
        reconcile_selection_and_volume(
            publisher,
            manager,
            sessions,
            selected,
            deadlines,
            selection_policy,
            selected_was_refreshed,
        );
    } else if timeline_refresh.changed && selected.id == Some(session_id) {
        publish_selected_timeline(publisher.app, publisher.snapshot, sessions, selected.id);
    }
}

/// 执行一次到期的音量重绑，并仅在仍未找到音频会话时安排下一档退避。
fn handle_volume_rebind_due<R: Runtime>(
    app: &AppHandle<R>,
    sessions: &[SessionEntry],
    selected: &mut SelectedMedia<R>,
    deadlines: &mut WorkerDeadlines,
    target_id: u64,
    attempt: usize,
) {
    if selected.id != Some(target_id) || selected.volume.snapshot().is_some() {
        return;
    }
    rebind_selected_volume(&mut selected.volume, sessions, target_id);
    selected.spectrum.bind(selected.volume.capture_process_id());
    let volume = selected.volume.snapshot();
    publish_volume(app, volume);
    if volume.is_none() {
        deadlines.schedule_volume_rebind(target_id, attempt.saturating_add(1));
    } else {
        deadlines.cancel_volume_rebind();
    }
}

fn non_empty_metadata(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_owned())
}

/// 从当前选择读取稳定来源标识，并交由窗口开关模块处理。
fn toggle_selected_player_window(
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

/// 获取 GSMTC 管理器并订阅当前会话与会话列表变化。
/// 同步当前全部 GSMTC 会话，并为新增会话建立独立事件订阅。
fn synchronize_sessions(
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

/// 管理器变化时补读所有轻量播放状态，兼容漏发单会话事件的播放器。
fn refresh_all_playback(
    entries: &mut [SessionEntry],
    next_activity_order: &mut u64,
    selected_id: Option<u64>,
) -> bool {
    entries.iter_mut().fold(false, |selected_changed, entry| {
        let changed = refresh_playback_entry(entry, next_activity_order);
        selected_changed || (changed && Some(entry.id) == selected_id)
    })
}

/// 为单个会话注册歌曲属性与播放状态事件。
/// 只刷新指定会话的歌曲元数据，避免播放事件重复读取和编码封面。
fn refresh_metadata(entries: &mut [SessionEntry], session_id: u64) -> MetadataRefresh {
    let Some(entry) = entries.iter_mut().find(|entry| entry.id == session_id) else {
        return MetadataRefresh::default();
    };
    let Ok(properties) = session::read_properties(&entry.registration.session)
        .inspect_err(|error| log::warn!("刷新媒体属性失败: {error}"))
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
fn timeline_pending_new_track(entries: &[SessionEntry], session_id: u64) -> bool {
    entries
        .iter()
        .find(|entry| entry.id == session_id)
        .is_some_and(|entry| entry.pending_previous_position_ms.is_some())
}

/// 标题已切换而时间线未出现新曲边界时，不向上层泄漏上一首的大进度。
fn reset_stale_timeline_at_track_boundary(
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
fn refresh_playback(
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
    let Ok(playback) = session::read_playback(&entry.registration.session)
        .inspect_err(|error| log::warn!("刷新媒体播放状态失败: {error}"))
    else {
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
fn refresh_timeline(entries: &mut [SessionEntry], session_id: u64) -> TimelineRefresh {
    let Some(entry) = entries.iter_mut().find(|entry| entry.id == session_id) else {
        return TimelineRefresh::default();
    };
    let timeline = session::read_timeline(&entry.registration.session).unwrap_or_else(|error| {
        log::warn!("刷新媒体时间线失败: {error}");
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

/// 按当前策略重新选择会话，仅在目标或已显示内容变化时广播。
fn reconcile_selection<R: Runtime>(
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
fn reconcile_selection_and_volume<R: Runtime>(
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

/// 按已选 GSMTC 来源绑定对应播放器的 Windows 应用音频会话。
fn bind_selected_volume(
    volume: &mut ApplicationVolumeController,
    deadlines: &mut WorkerDeadlines,
    entries: &[SessionEntry],
    selected_id: Option<u64>,
) {
    let Some(entry) = entries.iter().find(|entry| Some(entry.id) == selected_id) else {
        volume.bind(None, "", &[]);
        deadlines.cancel_volume_rebind();
        return;
    };
    let source_app_id = entry
        .registration
        .session
        .SourceAppUserModelId()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let player = identify(&source_app_id);
    volume.bind(selected_id, &source_app_id, player.executable_names());
    if volume.snapshot().is_none() {
        deadlines.schedule_volume_rebind(entry.id, 0);
    } else {
        deadlines.cancel_volume_rebind();
    }
}

/// Core Audio 通知会话集合变化后重新匹配当前播放器进程。
fn rebind_selected_volume(
    volume: &mut ApplicationVolumeController,
    entries: &[SessionEntry],
    target_id: u64,
) {
    let Some(entry) = entries.iter().find(|entry| entry.id == target_id) else {
        return;
    };
    let source_app_id = entry
        .registration
        .session
        .SourceAppUserModelId()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let player = identify(&source_app_id);
    volume.rebind(target_id, &source_app_id, player.executable_names());
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
fn normalize_selection_policy(
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
