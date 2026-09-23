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

use crate::{
    error::Error,
    media::{
        MediaRuntimeDiagnostics, MediaRuntimeSessionDiagnostics, MediaSessionSelectionPolicy,
        MediaSessionSnapshot, MediaSnapshotSubscriber, spectrum::AudioSpectrumController,
        system_volume::SystemVolumeController, volume::ApplicationVolumeController,
    },
};

use super::{
    MetadataRefresh, SelectedMedia, SessionEntry, TimelineRefresh, WorkerMessage,
    apartment::WinRtMta,
    channel::{WorkerReceiver, WorkerSender},
    deadlines::{ScheduledWorkerTask, WorkerDeadlines},
    metrics::{WorkerEnvelope, WorkerMessageKind, WorkerMetrics},
    playback::{
        refresh_all_playback, refresh_metadata, refresh_playback, refresh_timeline,
        reset_stale_timeline_at_track_boundary, timeline_pending_new_track,
    },
    publisher::{
        MediaSnapshotPublisher, publish_selected_timeline, publish_system_volume, publish_volume,
    },
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
    worker_metrics: WorkerMetrics,
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
    let mut state = WorkerState::new(
        &app,
        &manager.manager,
        sender,
        MediaSnapshotPublisher {
            app: &app,
            snapshot: &snapshot,
            subscriber: &snapshot_subscriber,
        },
        worker_metrics,
    );

    loop {
        state.run_due_tasks();
        match receive_message(&receiver, &state.deadlines) {
            IncomingMessage::Ready(envelope) => {
                state.metrics.record_received(&envelope);
                if let WorkerOutcome::Shutdown = state.handle_message(envelope.into_message()) {
                    break;
                }
            }
            // 等待超时只是为了回到循环顶部处理到期任务，没有消息需要消费。
            IncomingMessage::TimedOut => {}
            IncomingMessage::Disconnected => break,
        }
    }

    // 先释放会话表再释放管理器：两者都是 WinRT 对象，销毁顺序与创建顺序相反。
    drop(state);
}

enum IncomingMessage {
    Ready(WorkerEnvelope),
    /// 等待到期任务的时间窗结束，需要回到循环顶部。
    TimedOut,
    /// 所有发送端已断开，事件循环应当退出。
    Disconnected,
}

/// 等待下一条媒体消息；待处理的定时任务决定最长等待时间。
fn receive_message(receiver: &WorkerReceiver, deadlines: &WorkerDeadlines) -> IncomingMessage {
    let Some(timeout) = deadlines.next_timeout(Instant::now()) else {
        return match receiver.recv() {
            Ok(envelope) => IncomingMessage::Ready(envelope),
            Err(_) => IncomingMessage::Disconnected,
        };
    };
    match receiver.recv_timeout(timeout) {
        Ok(envelope) => IncomingMessage::Ready(envelope),
        Err(RecvTimeoutError::Timeout) => IncomingMessage::TimedOut,
        Err(RecvTimeoutError::Disconnected) => IncomingMessage::Disconnected,
    }
}

enum WorkerOutcome {
    Continue,
    Shutdown,
}

/// 媒体事件循环的独占状态。
///
/// 收敛成结构体是为了让每个事件处理器只关心自己那一件事：否则每次处理都要重复传同一组
/// 六七个参数，新增一个事件类型就要再抄一遍。
struct WorkerState<'a, R: Runtime> {
    manager: &'a GlobalSystemMediaTransportControlsSessionManager,
    sender: WorkerSender,
    publisher: MediaSnapshotPublisher<'a, R>,
    metrics: WorkerMetrics,
    sessions: Vec<SessionEntry>,
    selected: SelectedMedia<R>,
    deadlines: WorkerDeadlines,
    next_session_id: u64,
    next_activity_order: u64,
    selection_policy: MediaSessionSelectionPolicy,
    system_volume: Option<SystemVolumeController>,
}

impl<'a, R: Runtime> WorkerState<'a, R> {
    /// 建立循环状态，并完成首次会话同步与选择。
    fn new(
        app: &'a AppHandle<R>,
        manager: &'a GlobalSystemMediaTransportControlsSessionManager,
        sender: WorkerSender,
        publisher: MediaSnapshotPublisher<'a, R>,
        metrics: WorkerMetrics,
    ) -> Self {
        let selected = SelectedMedia {
            id: None,
            volume: ApplicationVolumeController::new(sender.clone()),
            spectrum: AudioSpectrumController::new(app.clone()),
        };
        let system_volume = SystemVolumeController::new(sender.clone())
            .inspect_err(|error| log::warn!("初始化 Windows 系统主音量失败: {error}"))
            .ok();
        let mut state = Self {
            manager,
            sender,
            publisher,
            metrics,
            sessions: Vec::new(),
            selected,
            deadlines: WorkerDeadlines::default(),
            next_session_id: 1,
            // 从 1 起：会话条目的 `activity_order` 用 0 表示“没有活动记录”，不能被真实序号占用。
            next_activity_order: 1,
            selection_policy: MediaSessionSelectionPolicy::default(),
            system_volume,
        };
        state.synchronize(false);
        state.reconcile(true);
        state
    }

    fn run_due_tasks(&mut self) {
        for task in self.deadlines.take_due(Instant::now()) {
            match task {
                ScheduledWorkerTask::MetadataSettle(session_id) => self.settle_metadata(session_id),
                ScheduledWorkerTask::VolumeRebind { target_id, attempt } => {
                    handle_volume_rebind_due(
                        self.publisher.app,
                        &self.sessions,
                        &mut self.selected,
                        &mut self.deadlines,
                        target_id,
                        attempt,
                    );
                }
            }
        }
    }

    fn handle_message(&mut self, message: WorkerMessage) -> WorkerOutcome {
        match message {
            WorkerMessage::EventsReady => self.handle_events_ready(),
            WorkerMessage::SelectionPolicyChanged(policy, result_sender) => {
                let normalized = normalize_selection_policy(policy);
                if self.selection_policy != normalized {
                    self.selection_policy = normalized;
                    self.synchronize(false);
                    self.reconcile(true);
                }
                let _ = result_sender.send(Ok(()));
            }
            WorkerMessage::Control(action, result_sender) => {
                let registration = self
                    .sessions
                    .iter()
                    .find(|entry| Some(entry.id) == self.selected.id)
                    .map(|entry| &entry.registration);
                let _ = result_sender.send(session::control(registration, action));
            }
            WorkerMessage::TogglePlayerWindow(result_sender) => {
                let _ = result_sender.send(toggle_selected_player_window(
                    &self.sessions,
                    self.selected.id,
                ));
            }
            WorkerMessage::GetVolume(result_sender) => {
                // 首次订阅可能晚于播放器启动；读取前补齐尚未就绪的音频绑定。
                if self.selected.volume.snapshot().is_none()
                    && let Some(target_id) = self.selected.id
                {
                    rebind_selected_volume(&mut self.selected.volume, &self.sessions, target_id);
                    self.refresh_volume_binding();
                }
                let _ = result_sender.send(self.selected.volume.snapshot());
            }
            WorkerMessage::SetVolume(level, result_sender) => {
                let result = self.selected.volume.set_level(level);
                if let Ok(next) = result {
                    publish_volume(self.publisher.app, Some(next));
                }
                let _ = result_sender.send(result);
            }
            WorkerMessage::ToggleMute(result_sender) => {
                let result = self.selected.volume.toggle_muted();
                if let Ok(next) = result {
                    publish_volume(self.publisher.app, Some(next));
                }
                let _ = result_sender.send(result);
            }
            WorkerMessage::GetSystemVolume(result_sender) => {
                let _ = result_sender.send(
                    self.system_volume
                        .as_ref()
                        .and_then(SystemVolumeController::snapshot),
                );
            }
            WorkerMessage::SetSystemVolume(level, result_sender) => {
                let result = self
                    .system_volume
                    .as_ref()
                    .ok_or_else(|| Error::Message("系统主音量控制当前不可用".to_owned()))
                    .and_then(|controller| controller.set_level(level));
                if let Ok(next) = result {
                    publish_system_volume(self.publisher.app, Some(next));
                }
                let _ = result_sender.send(result);
            }
            WorkerMessage::ToggleSystemMute(result_sender) => {
                let result = self
                    .system_volume
                    .as_ref()
                    .ok_or_else(|| Error::Message("系统主音量控制当前不可用".to_owned()))
                    .and_then(SystemVolumeController::toggle_muted);
                if let Ok(next) = result {
                    publish_system_volume(self.publisher.app, Some(next));
                }
                let _ = result_sender.send(result);
            }
            WorkerMessage::GetDiagnostics(result_sender) => {
                let _ = result_sender.send(self.diagnostics());
            }
            WorkerMessage::SpectrumEnabled(enabled, frame_rate, result_sender) => {
                let _ = result_sender.send(self.selected.spectrum.set_enabled(enabled, frame_rate));
            }
            WorkerMessage::Shutdown => return WorkerOutcome::Shutdown,
        }
        WorkerOutcome::Continue
    }

    /// 消费一次事件批次：先按管理器变化重建会话表，再按事件类型刷新对应会话。
    fn handle_events_ready(&mut self) {
        let events = self.sender.take_pending_events();
        if events.manager_changed {
            self.metrics
                .record_event_processed(WorkerMessageKind::ManagerChanged);
            self.handle_manager_changed();
        }
        for (session_id, observed_at) in events.media_properties_changed {
            self.metrics
                .record_event_processed(WorkerMessageKind::MediaPropertiesChanged);
            if self
                .deadlines
                .schedule_metadata_settle(session_id, observed_at)
            {
                self.metrics.record_coalesced_event();
            }
            self.metrics
                .observe_metadata_settle_pending(self.deadlines.metadata_settle_pending_count());
            self.settle_metadata(session_id);
        }
        for session_id in events.playback_info_changed {
            self.metrics
                .record_event_processed(WorkerMessageKind::PlaybackInfoChanged);
            self.handle_playback_info_changed(session_id);
        }
        for session_id in events.timeline_properties_changed {
            self.metrics
                .record_event_processed(WorkerMessageKind::TimelinePropertiesChanged);
            self.handle_timeline_properties_changed(session_id);
        }
        for target_id in events.volume_sessions_changed {
            self.metrics
                .record_event_processed(WorkerMessageKind::VolumeSessionsChanged);
            self.handle_volume_sessions_changed(target_id);
        }
        for target_id in events.volume_changed {
            self.metrics
                .record_event_processed(WorkerMessageKind::VolumeChanged);
            self.handle_volume_changed(target_id);
        }
        if events.system_volume_changed {
            self.metrics
                .record_event_processed(WorkerMessageKind::SystemVolumeChanged);
        }
        if events.default_audio_endpoint_changed {
            self.metrics
                .record_event_processed(WorkerMessageKind::DefaultAudioEndpointChanged);
            if let Some(controller) = self.system_volume.as_mut() {
                controller.rebind_default_endpoint();
                publish_system_volume(self.publisher.app, controller.snapshot());
            }
        } else if events.system_volume_changed {
            publish_system_volume(
                self.publisher.app,
                self.system_volume
                    .as_ref()
                    .and_then(SystemVolumeController::snapshot),
            );
        }
    }

    /// 媒体会话管理器换代：重建会话表并按播放状态重新排序。
    fn handle_manager_changed(&mut self) {
        self.synchronize(true);
        let selected_was_refreshed = refresh_all_playback(
            &mut self.sessions,
            &mut self.next_activity_order,
            self.selected.id,
        );
        self.reconcile(selected_was_refreshed);
    }

    /// 播放状态变化：必要时同步元数据与时间线，再决定是否重新选择。
    fn handle_playback_info_changed(&mut self, session_id: u64) {
        let playback_changed = refresh_playback(
            &mut self.sessions,
            session_id,
            &mut self.next_activity_order,
        );
        let metadata = if playback_changed {
            refresh_metadata(&mut self.sessions, session_id)
        } else {
            MetadataRefresh::default()
        };
        let pending_before = timeline_pending_new_track(&self.sessions, session_id);
        let timeline = refresh_timeline(&mut self.sessions, session_id);
        let track_confirmed =
            self.settle_timeline_boundary(session_id, metadata, timeline, pending_before);
        self.finish_session_refresh(
            session_id,
            playback_changed
                || metadata.changed
                || timeline.availability_changed
                || track_confirmed,
            timeline.changed,
        );
    }

    /// 时间线变化：时间线自身出现新曲边界时补齐元数据。
    fn handle_timeline_properties_changed(&mut self, session_id: u64) {
        let pending_before = timeline_pending_new_track(&self.sessions, session_id);
        let timeline = refresh_timeline(&mut self.sessions, session_id);
        let metadata = if timeline.track_boundary {
            refresh_metadata(&mut self.sessions, session_id)
        } else {
            MetadataRefresh::default()
        };
        let track_confirmed =
            self.settle_timeline_boundary(session_id, metadata, timeline, pending_before);
        self.finish_session_refresh(
            session_id,
            metadata.changed
                || timeline.availability_changed
                || timeline.track_boundary
                || track_confirmed,
            timeline.changed,
        );
    }

    /// 元数据结算：元数据与时间线都已稳定，按需重选或只刷新时间线。
    fn settle_metadata(&mut self, session_id: u64) {
        let pending_before = timeline_pending_new_track(&self.sessions, session_id);
        let metadata = refresh_metadata(&mut self.sessions, session_id);
        let timeline = refresh_timeline(&mut self.sessions, session_id);
        let track_confirmed =
            self.settle_timeline_boundary(session_id, metadata, timeline, pending_before);
        self.finish_session_refresh(
            session_id,
            metadata.changed || timeline.availability_changed || track_confirmed,
            timeline.changed,
        );
    }

    /// 播放器的音频会话重建：先补齐音量绑定再发布。
    fn handle_volume_sessions_changed(&mut self, target_id: u64) {
        if self.selected.id != Some(target_id) {
            return;
        }
        rebind_selected_volume(&mut self.selected.volume, &self.sessions, target_id);
        self.refresh_volume_binding();
    }

    /// 播放器音量变化：绑定已就绪，只需重新发布。
    fn handle_volume_changed(&mut self, target_id: u64) {
        if self.selected.id != Some(target_id) {
            return;
        }
        self.refresh_volume_binding();
    }

    /// 修正切歌瞬间的陈旧时间线，并返回时间线是否刚刚确认了新曲。
    fn settle_timeline_boundary(
        &mut self,
        session_id: u64,
        metadata: MetadataRefresh,
        timeline: TimelineRefresh,
        pending_before: bool,
    ) -> bool {
        reset_stale_timeline_at_track_boundary(
            &mut self.sessions,
            session_id,
            metadata.track_boundary,
            timeline.track_boundary,
        );
        // 时间线刚确认新曲：必须重新发布一次完整快照，否则歌词层拿不到此前被推迟的通知。
        pending_before && !timeline_pending_new_track(&self.sessions, session_id)
    }

    /// 会话刷新后的统一收尾：需要时重新选择，否则只把时间线推给任务栏。
    fn finish_session_refresh(
        &mut self,
        session_id: u64,
        needs_reconcile: bool,
        timeline_changed: bool,
    ) {
        if needs_reconcile {
            self.reconcile(self.selected.id == Some(session_id));
        } else if timeline_changed && self.selected.id == Some(session_id) {
            self.publish_timeline();
        }
    }

    /// 同步 GSMTC 会话表；新增会话会带上编号与活动顺序。
    fn synchronize(&mut self, new_sessions: bool) {
        synchronize_sessions(
            self.manager,
            &self.sender,
            &mut self.sessions,
            &mut self.next_session_id,
            &mut self.next_activity_order,
            new_sessions,
            self.selection_policy.only_supported_players,
        );
    }

    /// 重新选择当前媒体目标并绑定音量；`selected_was_refreshed` 决定是否保留已发布的快照。
    fn reconcile(&mut self, selected_was_refreshed: bool) {
        reconcile_selection_and_volume(
            &self.publisher,
            self.manager,
            &self.sessions,
            &mut self.selected,
            &mut self.deadlines,
            &self.selection_policy,
            selected_was_refreshed,
        );
    }

    /// 绑定音频捕获进程并重新发布音量；已拿到音量时撤销待重试的重新绑定。
    fn refresh_volume_binding(&mut self) {
        if self.selected.volume.snapshot().is_some() {
            self.deadlines.cancel_volume_rebind();
        }
        self.selected
            .spectrum
            .bind(self.selected.volume.capture_process_id());
        publish_volume(self.publisher.app, self.selected.volume.snapshot());
    }

    /// 只推送已选会话的轻量时间线，不重新序列化封面。
    fn publish_timeline(&self) {
        publish_selected_timeline(
            self.publisher.app,
            self.publisher.snapshot,
            &self.sessions,
            self.selected.id,
        );
    }

    /// 汇总当前媒体运行状态，供设置页诊断展示。
    fn diagnostics(&self) -> MediaRuntimeDiagnostics {
        let (spectrum_enabled, spectrum_active) = self.selected.spectrum.diagnostics();
        MediaRuntimeDiagnostics {
            session_count: self.sessions.len(),
            sessions: self
                .sessions
                .iter()
                .map(|entry| MediaRuntimeSessionDiagnostics {
                    player: entry.snapshot.player,
                    playback_status: entry.snapshot.playback.status,
                    title: non_empty_metadata(&entry.snapshot.metadata.title),
                    artist: non_empty_metadata(&entry.snapshot.metadata.artist),
                    timeline_available: entry.snapshot.timeline.is_some(),
                    selected: self.selected.id == Some(entry.id),
                })
                .collect(),
            selection_strategy: self.selection_policy.strategy,
            volume: self.selected.volume.snapshot(),
            audio_process_id: self.selected.volume.capture_process_id(),
            spectrum_enabled,
            spectrum_active,
            worker: self
                .metrics
                .snapshot(self.deadlines.metadata_settle_pending_count()),
        }
    }
}
