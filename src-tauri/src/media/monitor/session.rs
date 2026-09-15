//! GSMTC 管理器和单会话事件订阅。

use std::{sync::mpsc::Sender, thread, time::Duration};

use windows::{
    Foundation::TypedEventHandler,
    Media::Control::{
        CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager, MediaPropertiesChangedEventArgs,
        PlaybackInfoChangedEventArgs, SessionsChangedEventArgs, TimelinePropertiesChangedEventArgs,
    },
};

use super::WorkerMessage;
use crate::media::{
    MediaControlAction, MediaMetadata, MediaPlayback, MediaPlaybackControls, MediaSessionSnapshot,
    MediaTimeline, players::identify, source_icon::read_source_icon_data_url,
    thumbnail::read_thumbnail_data_url,
};

const METADATA_SETTLE_DELAY: Duration = Duration::from_millis(300);
const TICKS_PER_MILLISECOND: i64 = 10_000;

pub(super) struct ManagerRegistration {
    pub(super) manager: GlobalSystemMediaTransportControlsSessionManager,
    current_session_changed_token: i64,
    sessions_changed_token: i64,
}

impl Drop for ManagerRegistration {
    /// 释放管理器级事件订阅，避免回调晚于 worker 生命周期。
    fn drop(&mut self) {
        let _ = self
            .manager
            .RemoveCurrentSessionChanged(self.current_session_changed_token);
        let _ = self
            .manager
            .RemoveSessionsChanged(self.sessions_changed_token);
    }
}

pub(super) struct SessionRegistration {
    pub(super) session: GlobalSystemMediaTransportControlsSession,
    media_properties_changed_token: i64,
    playback_info_changed_token: i64,
    timeline_properties_changed_token: Option<i64>,
}

impl Drop for SessionRegistration {
    /// 释放单会话全部事件订阅。
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

/// 获取 GSMTC 管理器并订阅当前会话与会话列表变化。
pub(super) fn register_manager(
    sender: &Sender<WorkerMessage>,
) -> windows::core::Result<ManagerRegistration> {
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

/// 为单个会话注册歌曲属性与播放状态事件。
pub(super) fn bind_session(
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

/// 从当前会话生成完整、可序列化的前端快照。
pub(super) fn read_snapshot(
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
pub(super) fn read_metadata(
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
pub(super) fn read_playback(
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
pub(super) fn read_timeline(
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

/// 调用 GSMTC 官方控制方法，并保留播放器拒绝请求时的 false 结果。
pub(super) fn control(
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
