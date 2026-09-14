use std::{
    sync::{
        Arc, RwLock,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Emitter, Manager, Runtime};
use windows::{
    Foundation::TypedEventHandler,
    Media::Control::{
        CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager, MediaPropertiesChangedEventArgs,
        PlaybackInfoChangedEventArgs, SessionsChangedEventArgs, TimelinePropertiesChangedEventArgs,
    },
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
};

use super::{
    MediaControlAction, MediaMetadata, MediaPlayback, MediaPlaybackControls, MediaPlayer,
    MediaSessionSelectionPolicy, MediaSessionSnapshot, MediaTimeline, MediaVolumeSnapshot,
    model::MediaPlaybackStatus,
    players::{identify, selection_hold_after_title_change},
    selector::{SelectionCandidate, select_session},
    source_icon::read_source_icon_data_url,
    spectrum::AudioSpectrumController,
    thumbnail::read_thumbnail_data_url,
    volume::ApplicationVolumeController,
};

pub(super) const MEDIA_SESSION_CHANGED_EVENT: &str = "media://session-changed";
pub(super) const MEDIA_TIMELINE_CHANGED_EVENT: &str = "media://timeline-changed";
pub(super) const MEDIA_VOLUME_CHANGED_EVENT: &str = "media://volume-changed";
const WORKER_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
const DIAGNOSTICS_RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);
const METADATA_SETTLE_DELAY: Duration = Duration::from_millis(300);
const TICKS_PER_MILLISECOND: i64 = 10_000;

pub(super) enum WorkerMessage {
    ManagerChanged,
    MediaPropertiesChanged(u64),
    PlaybackInfoChanged(u64),
    TimelinePropertiesChanged(u64),
    SelectionPolicyChanged(
        MediaSessionSelectionPolicy,
        mpsc::SyncSender<Result<(), String>>,
    ),
    Control(MediaControlAction, mpsc::SyncSender<Result<bool, String>>),
    GetVolume(mpsc::SyncSender<Option<MediaVolumeSnapshot>>),
    SetVolume(f32, mpsc::SyncSender<Result<MediaVolumeSnapshot, String>>),
    ToggleMute(mpsc::SyncSender<Result<MediaVolumeSnapshot, String>>),
    GetDiagnostics(mpsc::SyncSender<super::MediaRuntimeDiagnostics>),
    SpectrumEnabled(bool, mpsc::SyncSender<Result<(), String>>),
    VolumeChanged(u64),
    VolumeSessionsChanged(u64),
    Shutdown,
}

/// 面向 Tauri command 的线程安全媒体服务句柄。
#[derive(Clone)]
pub struct MediaService {
    inner: Arc<MediaServiceInner>,
}

struct MediaServiceInner {
    sender: Sender<WorkerMessage>,
    snapshot: Arc<RwLock<Option<MediaSessionSnapshot>>>,
}

impl MediaService {
    /// 启动独立 WinRT MTA 线程，避免媒体 API 阻塞 Tauri 主线程。
    pub fn initialize<R: Runtime>(app: AppHandle<R>) -> Result<Self, std::io::Error> {
        let (sender, receiver) = mpsc::channel();
        let snapshot = Arc::new(RwLock::new(None));
        let worker_sender = sender.clone();
        let worker_snapshot = Arc::clone(&snapshot);

        thread::Builder::new()
            .name("media-session-monitor".to_owned())
            .spawn(move || run_worker(app, worker_sender, receiver, worker_snapshot))?;

        Ok(Self {
            inner: Arc::new(MediaServiceInner { sender, snapshot }),
        })
    }

    /// 返回最近发布的媒体会话快照。
    pub fn snapshot(&self) -> Option<MediaSessionSnapshot> {
        self.inner
            .snapshot
            .read()
            .ok()
            .and_then(|value| value.clone())
    }

    /// 在持有读锁期间只复制诊断所需文本，跳过可能很大的 Base64 图片。
    pub(crate) fn diagnostics_snapshot(&self) -> Option<super::MediaSnapshotDiagnostics> {
        self.inner.snapshot.read().ok().and_then(|snapshot| {
            snapshot
                .as_ref()
                .map(|snapshot| super::MediaSnapshotDiagnostics {
                    player: snapshot.player,
                    playback_status: snapshot.playback.status,
                    title: snapshot.metadata.title.clone(),
                    artist: snapshot.metadata.artist.clone(),
                    timeline: snapshot.timeline.clone(),
                    controls: snapshot.playback.controls,
                })
        })
    }

    /// 从媒体线程读取会话选择、应用音频和频谱绑定状态。
    pub(crate) fn runtime_diagnostics(&self) -> Result<super::MediaRuntimeDiagnostics, String> {
        let (result_sender, result_receiver) = mpsc::sync_channel(1);
        self.inner
            .sender
            .send(WorkerMessage::GetDiagnostics(result_sender))
            .map_err(|_| "媒体会话监控线程不可用".to_owned())?;
        result_receiver
            .recv_timeout(DIAGNOSTICS_RESPONSE_TIMEOUT)
            .map_err(|_| "媒体会话未返回诊断状态".to_owned())
    }

    /// 将控制请求串行投递给持有当前 WinRT 会话的线程。
    pub fn control(&self, action: MediaControlAction) -> Result<bool, String> {
        let (result_sender, result_receiver) = mpsc::sync_channel(1);
        self.inner
            .sender
            .send(WorkerMessage::Control(action, result_sender))
            .map_err(|_| "媒体会话监控线程不可用".to_owned())?;
        result_receiver
            .recv_timeout(WORKER_RESPONSE_TIMEOUT)
            .map_err(|_| "媒体会话未返回控制结果".to_owned())?
    }

    /// 更新多播放器会话选择策略，并立即重新计算控制目标。
    pub fn set_selection_policy(&self, policy: MediaSessionSelectionPolicy) -> Result<(), String> {
        let (result_sender, result_receiver) = mpsc::sync_channel(1);
        self.inner
            .sender
            .send(WorkerMessage::SelectionPolicyChanged(policy, result_sender))
            .map_err(|_| "媒体会话监控线程不可用".to_owned())?;
        result_receiver
            .recv_timeout(WORKER_RESPONSE_TIMEOUT)
            .map_err(|_| "媒体会话未返回策略更新结果".to_owned())?
    }

    /// 返回当前播放器的 Windows 单应用音量。
    pub fn volume(&self) -> Result<Option<MediaVolumeSnapshot>, String> {
        let (result_sender, result_receiver) = mpsc::sync_channel(1);
        self.inner
            .sender
            .send(WorkerMessage::GetVolume(result_sender))
            .map_err(|_| "媒体会话监控线程不可用".to_owned())?;
        result_receiver
            .recv_timeout(WORKER_RESPONSE_TIMEOUT)
            .map_err(|_| "媒体会话未返回应用音量".to_owned())
    }

    /// 设置当前播放器的 Windows 单应用音量。
    pub fn set_volume(&self, level: f32) -> Result<MediaVolumeSnapshot, String> {
        let (result_sender, result_receiver) = mpsc::sync_channel(1);
        self.inner
            .sender
            .send(WorkerMessage::SetVolume(level, result_sender))
            .map_err(|_| "媒体会话监控线程不可用".to_owned())?;
        result_receiver
            .recv_timeout(WORKER_RESPONSE_TIMEOUT)
            .map_err(|_| "媒体会话未返回音量设置结果".to_owned())?
    }

    /// 切换当前播放器的 Windows 单应用静音状态。
    pub fn toggle_mute(&self) -> Result<MediaVolumeSnapshot, String> {
        let (result_sender, result_receiver) = mpsc::sync_channel(1);
        self.inner
            .sender
            .send(WorkerMessage::ToggleMute(result_sender))
            .map_err(|_| "媒体会话监控线程不可用".to_owned())?;
        result_receiver
            .recv_timeout(WORKER_RESPONSE_TIMEOUT)
            .map_err(|_| "媒体会话未返回静音切换结果".to_owned())?
    }

    /// 启用或停止当前播放器的真实音频频谱采集。
    pub fn set_spectrum_enabled(&self, enabled: bool) -> Result<(), String> {
        let (result_sender, result_receiver) = mpsc::sync_channel(1);
        self.inner
            .sender
            .send(WorkerMessage::SpectrumEnabled(enabled, result_sender))
            .map_err(|_| "媒体会话监控线程不可用".to_owned())?;
        result_receiver
            .recv_timeout(WORKER_RESPONSE_TIMEOUT)
            .map_err(|_| "媒体会话未返回频谱开关结果".to_owned())?
    }
}

impl Drop for MediaServiceInner {
    fn drop(&mut self) {
        let _ = self.sender.send(WorkerMessage::Shutdown);
    }
}

struct ManagerRegistration {
    manager: GlobalSystemMediaTransportControlsSessionManager,
    current_session_changed_token: i64,
    sessions_changed_token: i64,
}

impl Drop for ManagerRegistration {
    fn drop(&mut self) {
        let _ = self
            .manager
            .RemoveCurrentSessionChanged(self.current_session_changed_token);
        let _ = self
            .manager
            .RemoveSessionsChanged(self.sessions_changed_token);
    }
}

struct SessionRegistration {
    session: GlobalSystemMediaTransportControlsSession,
    media_properties_changed_token: i64,
    playback_info_changed_token: i64,
    timeline_properties_changed_token: Option<i64>,
}

/// 单个 GSMTC 会话的事件注册、快照与最近播放序号。
struct SessionEntry {
    id: u64,
    registration: SessionRegistration,
    snapshot: MediaSessionSnapshot,
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

impl Drop for SessionRegistration {
    fn drop(&mut self) {
        let _ = self
            .session
            .RemoveMediaPropertiesChanged(self.media_properties_changed_token);
        let _ = self
            .session
            .RemovePlaybackInfoChanged(self.playback_info_changed_token);
        if let Some(token) = self.timeline_properties_changed_token {
            let _ = self.session.RemoveTimelinePropertiesChanged(token);
        }
    }
}

/// 初始化并运行串行事件循环；轮询不参与媒体状态同步。
fn run_worker<R: Runtime>(
    app: AppHandle<R>,
    sender: Sender<WorkerMessage>,
    receiver: Receiver<WorkerMessage>,
    snapshot: Arc<RwLock<Option<MediaSessionSnapshot>>>,
) {
    // SAFETY: 此处运行在新建专用线程，成功初始化后在线程退出前成对调用 RoUninitialize。
    if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        log::error!("初始化媒体会话 WinRT 线程失败: {error}");
        return;
    }

    let manager = match register_manager(&sender) {
        Ok(manager) => manager,
        Err(error) => {
            log::error!("连接 Windows 媒体会话管理器失败: {error}");
            // SAFETY: 本线程上方的 RoInitialize 已成功。
            unsafe { RoUninitialize() };
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
        &app,
        &snapshot,
        &manager.manager,
        &sessions,
        &mut selected,
        &selection_policy,
        true,
    );

    while let Ok(message) = receiver.recv() {
        match message {
            WorkerMessage::ManagerChanged => {
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
                    &app,
                    &snapshot,
                    &manager.manager,
                    &sessions,
                    &mut selected,
                    &selection_policy,
                    selected_was_refreshed,
                );
            }
            WorkerMessage::MediaPropertiesChanged(session_id) => {
                let metadata_refresh = refresh_metadata(&mut sessions, session_id);
                let timeline_refresh = refresh_timeline(&mut sessions, session_id);
                reset_stale_timeline_at_track_boundary(
                    &mut sessions,
                    session_id,
                    metadata_refresh.track_boundary,
                    timeline_refresh.track_boundary,
                );
                if metadata_refresh.changed || timeline_refresh.availability_changed {
                    let selected_was_refreshed = selected.id == Some(session_id);
                    reconcile_selection_and_volume(
                        &app,
                        &snapshot,
                        &manager.manager,
                        &sessions,
                        &mut selected,
                        &selection_policy,
                        selected_was_refreshed,
                    );
                } else if timeline_refresh.changed && selected.id == Some(session_id) {
                    publish_selected_timeline(&app, &snapshot, &sessions, selected.id);
                }
            }
            WorkerMessage::PlaybackInfoChanged(session_id) => {
                let playback_changed =
                    refresh_playback(&mut sessions, session_id, &mut next_activity_order);
                let metadata_refresh = if playback_changed {
                    refresh_metadata(&mut sessions, session_id)
                } else {
                    MetadataRefresh::default()
                };
                let timeline_refresh = refresh_timeline(&mut sessions, session_id);
                reset_stale_timeline_at_track_boundary(
                    &mut sessions,
                    session_id,
                    metadata_refresh.track_boundary,
                    timeline_refresh.track_boundary,
                );
                if playback_changed
                    || metadata_refresh.changed
                    || timeline_refresh.availability_changed
                {
                    let selected_was_refreshed = selected.id == Some(session_id);
                    reconcile_selection_and_volume(
                        &app,
                        &snapshot,
                        &manager.manager,
                        &sessions,
                        &mut selected,
                        &selection_policy,
                        selected_was_refreshed,
                    );
                } else if timeline_refresh.changed && selected.id == Some(session_id) {
                    publish_selected_timeline(&app, &snapshot, &sessions, selected.id);
                }
            }
            WorkerMessage::TimelinePropertiesChanged(session_id) => {
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
                if metadata_refresh.changed
                    || timeline_refresh.availability_changed
                    || timeline_refresh.track_boundary
                {
                    let selected_was_refreshed = selected.id == Some(session_id);
                    reconcile_selection_and_volume(
                        &app,
                        &snapshot,
                        &manager.manager,
                        &sessions,
                        &mut selected,
                        &selection_policy,
                        selected_was_refreshed,
                    );
                } else if timeline_refresh.changed && selected.id == Some(session_id) {
                    publish_selected_timeline(&app, &snapshot, &sessions, selected.id);
                }
            }
            WorkerMessage::SelectionPolicyChanged(policy, result_sender) => {
                selection_policy = normalize_selection_policy(policy);
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
                    &app,
                    &snapshot,
                    &manager.manager,
                    &sessions,
                    &mut selected,
                    &selection_policy,
                    true,
                );
                let _ = result_sender.send(Ok(()));
            }
            WorkerMessage::Control(action, result_sender) => {
                let result = control_session(
                    sessions
                        .iter()
                        .find(|entry| Some(entry.id) == selected.id)
                        .map(|entry| &entry.registration),
                    action,
                );
                let _ = result_sender.send(result);
            }
            WorkerMessage::GetVolume(result_sender) => {
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
                });
            }
            WorkerMessage::SpectrumEnabled(enabled, result_sender) => {
                let result = selected.spectrum.set_enabled(enabled);
                let _ = result_sender.send(result);
            }
            WorkerMessage::VolumeChanged(target_id) => {
                if selected.id == Some(target_id) {
                    selected.spectrum.bind(selected.volume.capture_process_id());
                    publish_volume(&app, selected.volume.snapshot());
                }
            }
            WorkerMessage::VolumeSessionsChanged(target_id) => {
                if selected.id == Some(target_id) {
                    rebind_selected_volume(&mut selected.volume, &sessions, target_id);
                    selected.spectrum.bind(selected.volume.capture_process_id());
                    publish_volume(&app, selected.volume.snapshot());
                }
            }
            WorkerMessage::Shutdown => break,
        }
    }

    drop(sessions);
    drop(manager);
    // SAFETY: 本线程上的 RoInitialize 已成功，且 WinRT 对象和事件处理器均已释放。
    unsafe { RoUninitialize() };
}

fn non_empty_metadata(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_owned())
}

/// 获取 GSMTC 管理器并订阅当前会话与会话列表变化。
fn register_manager(sender: &Sender<WorkerMessage>) -> windows::core::Result<ManagerRegistration> {
    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
    let current_sender = sender.clone();
    let current_session_changed_token =
        manager.CurrentSessionChanged(&TypedEventHandler::<
            GlobalSystemMediaTransportControlsSessionManager,
            CurrentSessionChangedEventArgs,
        >::new(move |_, _| {
            let _ = current_sender.send(WorkerMessage::ManagerChanged);
            Ok(())
        }))?;
    let sessions_sender = sender.clone();
    let sessions_changed_token = match manager.SessionsChanged(&TypedEventHandler::<
        GlobalSystemMediaTransportControlsSessionManager,
        SessionsChangedEventArgs,
    >::new(move |_, _| {
        let _ = sessions_sender.send(WorkerMessage::ManagerChanged);
        Ok(())
    })) {
        Ok(token) => token,
        Err(error) => {
            let _ = manager.RemoveCurrentSessionChanged(current_session_changed_token);
            return Err(error);
        }
    };

    Ok(ManagerRegistration {
        manager,
        current_session_changed_token,
        sessions_changed_token,
    })
}

/// 同步当前全部 GSMTC 会话，并为新增会话建立独立事件订阅。
fn synchronize_sessions(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    sender: &Sender<WorkerMessage>,
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
        let Some(registration) = bind_session(session, id, sender) else {
            continue;
        };
        let Ok(snapshot) = read_snapshot(&registration.session) else {
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
fn bind_session(
    session: GlobalSystemMediaTransportControlsSession,
    session_id: u64,
    sender: &Sender<WorkerMessage>,
) -> Option<SessionRegistration> {
    let metadata_sender = sender.clone();
    let media_properties_changed_token = session
        .MediaPropertiesChanged(&TypedEventHandler::<
            GlobalSystemMediaTransportControlsSession,
            MediaPropertiesChangedEventArgs,
        >::new(move |_, _| {
            let _ = metadata_sender.send(WorkerMessage::MediaPropertiesChanged(session_id));
            let settled_sender = metadata_sender.clone();
            let _ = thread::Builder::new()
                .name("media-metadata-settle".to_owned())
                .spawn(move || {
                    thread::sleep(METADATA_SETTLE_DELAY);
                    let _ = settled_sender.send(WorkerMessage::MediaPropertiesChanged(session_id));
                });
            Ok(())
        }))
        .ok()?;
    let playback_sender = sender.clone();
    let playback_info_changed_token = match session.PlaybackInfoChanged(&TypedEventHandler::<
        GlobalSystemMediaTransportControlsSession,
        PlaybackInfoChangedEventArgs,
    >::new(move |_, _| {
        let _ = playback_sender.send(WorkerMessage::PlaybackInfoChanged(session_id));
        Ok(())
    })) {
        Ok(token) => token,
        Err(_) => {
            let _ = session.RemoveMediaPropertiesChanged(media_properties_changed_token);
            return None;
        }
    };
    let timeline_sender = sender.clone();
    let timeline_properties_changed_token = session
        .TimelinePropertiesChanged(&TypedEventHandler::<
            GlobalSystemMediaTransportControlsSession,
            TimelinePropertiesChangedEventArgs,
        >::new(move |_, _| {
            let _ = timeline_sender.send(WorkerMessage::TimelinePropertiesChanged(session_id));
            Ok(())
        }))
        .ok();

    Some(SessionRegistration {
        session,
        media_properties_changed_token,
        playback_info_changed_token,
        timeline_properties_changed_token,
    })
}

/// 只刷新指定会话的歌曲元数据，避免播放事件重复读取和编码封面。
fn refresh_metadata(entries: &mut [SessionEntry], session_id: u64) -> MetadataRefresh {
    let Some(entry) = entries.iter_mut().find(|entry| entry.id == session_id) else {
        return MetadataRefresh::default();
    };
    let Ok(metadata) = read_metadata(&entry.registration.session)
        .inspect_err(|error| log::warn!("刷新媒体属性失败: {error}"))
    else {
        return MetadataRefresh::default();
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
    let Ok(playback) = read_playback(&entry.registration.session)
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
    let timeline = read_timeline(&entry.registration.session).unwrap_or_else(|error| {
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
    app: &AppHandle<R>,
    snapshot: &RwLock<Option<MediaSessionSnapshot>>,
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
        publish_selected_snapshot(app, snapshot, entries, next_id);
    }
    selection_changed
}

/// 统一处理会话选择与音量目标切换，避免各事件分支重复绑定逻辑。
fn reconcile_selection_and_volume<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &RwLock<Option<MediaSessionSnapshot>>,
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    entries: &[SessionEntry],
    selected: &mut SelectedMedia<R>,
    policy: &MediaSessionSelectionPolicy,
    force_publish: bool,
) {
    if reconcile_selection(
        app,
        snapshot,
        manager,
        entries,
        &mut selected.id,
        policy,
        force_publish,
    ) {
        bind_selected_volume(&mut selected.volume, entries, selected.id);
        selected.spectrum.bind(selected.volume.capture_process_id());
        publish_volume(app, selected.volume.snapshot());
    }
}

/// 按已选 GSMTC 来源绑定对应播放器的 Windows 应用音频会话。
fn bind_selected_volume(
    volume: &mut ApplicationVolumeController,
    entries: &[SessionEntry],
    selected_id: Option<u64>,
) {
    let Some(entry) = entries.iter().find(|entry| Some(entry.id) == selected_id) else {
        volume.bind(None, "", &[]);
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
    volume.schedule_initial_rebind();
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

/// 广播应用音量；悬浮窗未显示时事件只更新轻量前端状态。
fn publish_volume<R: Runtime>(app: &AppHandle<R>, volume: Option<MediaVolumeSnapshot>) {
    if let Err(error) = app.emit(MEDIA_VOLUME_CHANGED_EVENT, volume) {
        log::warn!("向任务栏广播播放器应用音量失败: {error}");
    }
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

/// 发布已选会话快照；不存在有效目标时清空任务栏媒体状态。
fn publish_selected_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &RwLock<Option<MediaSessionSnapshot>>,
    entries: &[SessionEntry],
    selected_id: Option<u64>,
) {
    let next = entries
        .iter()
        .find(|entry| Some(entry.id) == selected_id)
        .map(|entry| entry.snapshot.clone());
    publish_snapshot(app, snapshot, next);
}

/// 仅发布轻量时间线，避免播放器定期更新时间时重复序列化封面。
fn publish_selected_timeline<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &RwLock<Option<MediaSessionSnapshot>>,
    entries: &[SessionEntry],
    selected_id: Option<u64>,
) {
    let next = entries
        .iter()
        .find(|entry| Some(entry.id) == selected_id)
        .and_then(|entry| entry.snapshot.timeline.clone());
    if let Ok(mut current) = snapshot.write()
        && let Some(current) = current.as_mut()
    {
        current.timeline.clone_from(&next);
    }
    if let Err(error) = app.emit(MEDIA_TIMELINE_CHANGED_EVENT, &next) {
        log::warn!("向任务栏广播媒体时间线失败: {error}");
    }
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

/// 从当前会话生成完整、可序列化的前端快照。
fn read_snapshot(
    session: &GlobalSystemMediaTransportControlsSession,
) -> windows::core::Result<MediaSessionSnapshot> {
    let source_app_id = session
        .SourceAppUserModelId()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let identified = identify(&source_app_id);
    let metadata = read_metadata(session)?;

    Ok(MediaSessionSnapshot {
        source_icon_data_url: read_source_icon_data_url(
            &source_app_id,
            identified.executable_names(),
        ),
        player: identified.player,
        metadata,
        playback: read_playback(session).unwrap_or_default(),
        timeline: read_timeline(session).unwrap_or_default(),
    })
}

/// 读取播放器实际向 SMTC 发布的全部歌曲元数据。
fn read_metadata(
    session: &GlobalSystemMediaTransportControlsSession,
) -> windows::core::Result<MediaMetadata> {
    let properties = session.TryGetMediaPropertiesAsync()?.get()?;
    let thumbnail_data_url = properties
        .Thumbnail()
        .ok()
        .and_then(|thumbnail| read_thumbnail_data_url(&thumbnail).ok().flatten());

    Ok(MediaMetadata {
        title: properties
            .Title()
            .map(|value| value.to_string())
            .unwrap_or_default(),
        artist: properties
            .Artist()
            .map(|value| value.to_string())
            .unwrap_or_default(),
        album_artist: properties
            .AlbumArtist()
            .map(|value| value.to_string())
            .unwrap_or_default(),
        subtitle: properties
            .Subtitle()
            .map(|value| value.to_string())
            .unwrap_or_default(),
        thumbnail_data_url,
    })
}

/// 读取播放器声明的状态与基础控制能力。
fn read_playback(
    session: &GlobalSystemMediaTransportControlsSession,
) -> windows::core::Result<MediaPlayback> {
    let playback = session.GetPlaybackInfo()?;
    let controls = playback.Controls()?;
    Ok(MediaPlayback {
        status: playback
            .PlaybackStatus()
            .map(Into::into)
            .unwrap_or_default(),
        controls: MediaPlaybackControls {
            can_play: controls.IsPlayEnabled().unwrap_or_default(),
            can_pause: controls.IsPauseEnabled().unwrap_or_default(),
            can_toggle_play_pause: controls.IsPlayPauseToggleEnabled().unwrap_or_default(),
            can_skip_next: controls.IsNextEnabled().unwrap_or_default(),
            can_skip_previous: controls.IsPreviousEnabled().unwrap_or_default(),
        },
    })
}

/// 读取并校验 GSMTC 时间线；无有效起止区间时不向前端伪造进度。
fn read_timeline(
    session: &GlobalSystemMediaTransportControlsSession,
) -> windows::core::Result<Option<MediaTimeline>> {
    let timeline = session.GetTimelineProperties()?;
    let start_time_ms = timeline.StartTime()?.Duration / TICKS_PER_MILLISECOND;
    let declared_end_time_ms = timeline.EndTime()?.Duration / TICKS_PER_MILLISECOND;
    let declared_max_seek_time_ms = timeline.MaxSeekTime()?.Duration / TICKS_PER_MILLISECOND;
    let end_time_ms = declared_end_time_ms.max(declared_max_seek_time_ms);
    if end_time_ms <= start_time_ms {
        return Ok(None);
    }

    let playback = session.GetPlaybackInfo().ok();
    let controls = playback.as_ref().and_then(|value| value.Controls().ok());
    let min_seek_time_ms = timeline
        .MinSeekTime()?
        .Duration
        .div_euclid(TICKS_PER_MILLISECOND)
        .clamp(start_time_ms, end_time_ms);
    let max_seek_time_ms = declared_max_seek_time_ms.clamp(min_seek_time_ms, end_time_ms);
    let position_ms = timeline
        .Position()?
        .Duration
        .div_euclid(TICKS_PER_MILLISECOND)
        .clamp(start_time_ms, end_time_ms);
    let playback_rate = playback
        .and_then(|value| value.PlaybackRate().ok())
        .and_then(|value| value.Value().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1.0);

    Ok(Some(MediaTimeline {
        start_time_ms,
        end_time_ms,
        position_ms,
        min_seek_time_ms,
        max_seek_time_ms,
        playback_rate,
        can_seek: controls
            .and_then(|value| value.IsPlaybackPositionEnabled().ok())
            .unwrap_or_default()
            && max_seek_time_ms > min_seek_time_ms,
    }))
}

/// 原子替换缓存并把相同值广播给所有任务栏窗口。
fn publish_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &RwLock<Option<MediaSessionSnapshot>>,
    next: Option<MediaSessionSnapshot>,
) {
    if let Ok(mut current) = snapshot.write() {
        current.clone_from(&next);
    }
    emit_snapshot(app, &next);
    if let Some(lyrics) = app.try_state::<crate::lyrics::LyricsService>() {
        lyrics.update_media(next.as_ref());
    }
}

/// 广播媒体快照；窗口未就绪时由前端初始 command 补取缓存。
fn emit_snapshot<R: Runtime>(app: &AppHandle<R>, snapshot: &Option<MediaSessionSnapshot>) {
    if let Err(error) = app.emit(MEDIA_SESSION_CHANGED_EVENT, snapshot) {
        log::warn!("向任务栏广播媒体会话失败: {error}");
    }
}

/// 调用 GSMTC 官方控制方法，并保留播放器拒绝请求时的 false 结果。
fn control_session(
    registration: Option<&SessionRegistration>,
    action: MediaControlAction,
) -> Result<bool, String> {
    let session = &registration
        .ok_or_else(|| "当前没有可控制的 Windows 媒体会话".to_owned())?
        .session;
    let playback = session
        .GetPlaybackInfo()
        .map_err(|error| error.to_string())?;
    let controls = playback.Controls().map_err(|error| error.to_string())?;
    let operation = match action {
        MediaControlAction::TogglePlayPause => {
            if controls
                .IsPlayPauseToggleEnabled()
                .map_err(|error| error.to_string())?
            {
                session.TryTogglePlayPauseAsync()
            } else if playback
                .PlaybackStatus()
                .map_err(|error| error.to_string())?
                == windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing
            {
                if controls
                    .IsPauseEnabled()
                    .map_err(|error| error.to_string())?
                {
                    session.TryPauseAsync()
                } else {
                    return Ok(false);
                }
            } else if controls
                .IsPlayEnabled()
                .map_err(|error| error.to_string())?
            {
                session.TryPlayAsync()
            } else {
                return Ok(false);
            }
        }
        MediaControlAction::SkipNext => {
            if controls
                .IsNextEnabled()
                .map_err(|error| error.to_string())?
            {
                session.TrySkipNextAsync()
            } else {
                return Ok(false);
            }
        }
        MediaControlAction::SkipPrevious => {
            if controls
                .IsPreviousEnabled()
                .map_err(|error| error.to_string())?
            {
                session.TrySkipPreviousAsync()
            } else {
                return Ok(false);
            }
        }
    }
    .map_err(|error| error.to_string())?;
    operation.get().map_err(|error| error.to_string())
}
