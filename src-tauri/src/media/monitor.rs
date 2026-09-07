use std::{
    sync::{
        Arc, RwLock,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use tauri::{AppHandle, Emitter, Runtime};
use windows::{
    Foundation::TypedEventHandler,
    Media::Control::{
        CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager, MediaPropertiesChangedEventArgs,
        PlaybackInfoChangedEventArgs, SessionsChangedEventArgs,
    },
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
};

use super::{
    MediaControlAction, MediaMetadata, MediaPlayback, MediaPlaybackControls, MediaPlayer,
    MediaSessionSelectionPolicy, MediaSessionSnapshot,
    model::MediaPlaybackStatus,
    players::identify,
    selector::{SelectionCandidate, select_session},
    source_icon::read_source_icon_data_url,
    thumbnail::read_thumbnail_data_url,
};

pub(super) const MEDIA_SESSION_CHANGED_EVENT: &str = "media://session-changed";
const WORKER_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

enum WorkerMessage {
    ManagerChanged,
    MediaPropertiesChanged(u64),
    PlaybackInfoChanged(u64),
    SelectionPolicyChanged(
        MediaSessionSelectionPolicy,
        mpsc::SyncSender<Result<(), String>>,
    ),
    Control(MediaControlAction, mpsc::SyncSender<Result<bool, String>>),
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
}

/// 单个 GSMTC 会话的事件注册、快照与最近播放序号。
struct SessionEntry {
    id: u64,
    registration: SessionRegistration,
    snapshot: MediaSessionSnapshot,
    activity_order: u64,
}

impl Drop for SessionRegistration {
    fn drop(&mut self) {
        let _ = self
            .session
            .RemoveMediaPropertiesChanged(self.media_properties_changed_token);
        let _ = self
            .session
            .RemovePlaybackInfoChanged(self.playback_info_changed_token);
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
    let mut selected_id = None;
    let mut selection_policy = MediaSessionSelectionPolicy::default();
    synchronize_sessions(
        &manager.manager,
        &sender,
        &mut sessions,
        &mut next_session_id,
        &mut next_activity_order,
        false,
    );
    reconcile_selection(
        &app,
        &snapshot,
        &manager.manager,
        &sessions,
        &mut selected_id,
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
                );
                let selected_was_refreshed =
                    refresh_all_playback(&mut sessions, &mut next_activity_order, selected_id);
                reconcile_selection(
                    &app,
                    &snapshot,
                    &manager.manager,
                    &sessions,
                    &mut selected_id,
                    &selection_policy,
                    selected_was_refreshed,
                );
            }
            WorkerMessage::MediaPropertiesChanged(session_id) => {
                if refresh_metadata(&mut sessions, session_id) && selected_id == Some(session_id) {
                    publish_selected_snapshot(&app, &snapshot, &sessions, selected_id);
                }
            }
            WorkerMessage::PlaybackInfoChanged(session_id) => {
                let refreshed =
                    refresh_playback(&mut sessions, session_id, &mut next_activity_order);
                if refreshed {
                    let selected_was_refreshed = selected_id == Some(session_id);
                    reconcile_selection(
                        &app,
                        &snapshot,
                        &manager.manager,
                        &sessions,
                        &mut selected_id,
                        &selection_policy,
                        selected_was_refreshed,
                    );
                }
            }
            WorkerMessage::SelectionPolicyChanged(policy, result_sender) => {
                selection_policy = normalize_selection_policy(policy);
                reconcile_selection(
                    &app,
                    &snapshot,
                    &manager.manager,
                    &sessions,
                    &mut selected_id,
                    &selection_policy,
                    true,
                );
                let _ = result_sender.send(Ok(()));
            }
            WorkerMessage::Control(action, result_sender) => {
                let result = control_session(
                    sessions
                        .iter()
                        .find(|entry| Some(entry.id) == selected_id)
                        .map(|entry| &entry.registration),
                    action,
                );
                let _ = result_sender.send(result);
            }
            WorkerMessage::Shutdown => break,
        }
    }

    drop(sessions);
    drop(manager);
    // SAFETY: 本线程上的 RoInitialize 已成功，且 WinRT 对象和事件处理器均已释放。
    unsafe { RoUninitialize() };
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
) {
    let Ok(view) = manager.GetSessions() else {
        return;
    };
    let sessions = (0..view.Size().unwrap_or_default())
        .filter_map(|index| view.GetAt(index).ok())
        .collect::<Vec<_>>();

    entries.retain(|entry| sessions.contains(&entry.registration.session));

    for session in sessions {
        if entries
            .iter()
            .any(|entry| entry.registration.session == session)
        {
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
        });
    }
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

    Some(SessionRegistration {
        session,
        media_properties_changed_token,
        playback_info_changed_token,
    })
}

/// 只刷新指定会话的歌曲元数据，避免播放事件重复读取和编码封面。
fn refresh_metadata(entries: &mut [SessionEntry], session_id: u64) -> bool {
    let Some(entry) = entries.iter_mut().find(|entry| entry.id == session_id) else {
        return false;
    };
    let Ok(metadata) = read_metadata(&entry.registration.session)
        .inspect_err(|error| log::warn!("刷新媒体属性失败: {error}"))
    else {
        return false;
    };
    if entry.snapshot.metadata == metadata {
        return false;
    }
    entry.snapshot.metadata = metadata;
    true
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

/// 按当前策略重新选择会话，仅在目标或已显示内容变化时广播。
fn reconcile_selection<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &RwLock<Option<MediaSessionSnapshot>>,
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    entries: &[SessionEntry],
    selected_id: &mut Option<u64>,
    policy: &MediaSessionSelectionPolicy,
    force_publish: bool,
) {
    let windows_current = manager.GetCurrentSession().ok();
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
        })
        .collect::<Vec<_>>();
    let next_id = select_session(&candidates, *selected_id, policy);
    let selection_changed = next_id != *selected_id;
    *selected_id = next_id;

    if selection_changed || force_publish {
        publish_selected_snapshot(app, snapshot, entries, next_id);
    }
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
