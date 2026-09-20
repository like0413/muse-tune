//! 媒体事件循环：独占一个 WinRT MTA 线程，串行消费媒体事件与前台命令。
//!
//! 所有媒体状态只在这个线程里被修改，因此这里不需要为会话表或选择结果加锁；
//! 外部只能通过消息通道请求，避免 WinRT 的线程亲和性被破坏。

use std::{
    sync::{Arc, RwLock, mpsc::RecvTimeoutError},
    time::Instant,
};

use tauri::{AppHandle, Runtime};
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

use crate::media::{
    MediaRuntimeDiagnostics, MediaRuntimeSessionDiagnostics, MediaSessionSelectionPolicy,
    MediaSessionSnapshot, MediaSnapshotSubscriber, spectrum::AudioSpectrumController,
    volume::ApplicationVolumeController,
};

use super::{
    MetadataRefresh, SelectedMedia, SessionEntry, WorkerMessage,
    apartment::WinRtMta,
    deadlines::{ScheduledWorkerTask, WorkerDeadlines},
    metrics::{WorkerMessageKind, WorkerMetrics, WorkerReceiver, WorkerSender},
    playback::{
        refresh_all_playback, refresh_metadata, refresh_playback, refresh_timeline,
        reset_stale_timeline_at_track_boundary, timeline_pending_new_track,
    },
    publisher::{MediaSnapshotPublisher, publish_selected_timeline, publish_volume},
    registry::{non_empty_metadata, synchronize_sessions},
    selection::{
        normalize_selection_policy, reconcile_selection_and_volume, toggle_selected_player_window,
    },
    session,
    volume_binding::{handle_volume_rebind_due, rebind_selected_volume},
};

/// 初始化并运行串行事件循环；轮询不参与媒体状态同步。
pub(super) fn run_worker<R: Runtime>(
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
                let _ = result_sender.send(MediaRuntimeDiagnostics {
                    session_count: sessions.len(),
                    sessions: sessions
                        .iter()
                        .map(|entry| MediaRuntimeSessionDiagnostics {
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
#[allow(clippy::too_many_arguments)]
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
