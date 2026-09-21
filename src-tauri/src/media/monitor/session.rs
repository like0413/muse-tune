//! GSMTC 管理器和单会话事件订阅。

use std::time::Instant;

use windows::{
    Foundation::TypedEventHandler,
    Media::Control::{
        CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager, MediaPropertiesChangedEventArgs,
        PlaybackInfoChangedEventArgs, SessionsChangedEventArgs, TimelinePropertiesChangedEventArgs,
    },
    Storage::Streams::IRandomAccessStreamReference,
};

use super::{metrics::WorkerSender, pending_events::WorkerEvent};
use crate::error::Error;
use crate::media::{
    MediaControlAction, MediaMetadata, MediaPlayback, MediaPlaybackControls, MediaSessionSnapshot,
    MediaTimeline, players::identify, source_icon::read_source_icon_data_url,
    thumbnail::read_thumbnail_data_url,
};

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
    sender: &WorkerSender,
) -> windows::core::Result<ManagerRegistration> {
    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
    let current_sender = sender.clone();
    let current_session_changed_token =
        manager.CurrentSessionChanged(&TypedEventHandler::<
            GlobalSystemMediaTransportControlsSessionManager,
            CurrentSessionChangedEventArgs,
        >::new(move |_, _| {
            current_sender.send_event(WorkerEvent::Manager);
            Ok(())
        }))?;
    let sessions_sender = sender.clone();
    let sessions_changed_token = match manager.SessionsChanged(&TypedEventHandler::<
        GlobalSystemMediaTransportControlsSessionManager,
        SessionsChangedEventArgs,
    >::new(move |_, _| {
        sessions_sender.send_event(WorkerEvent::Manager);
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
    sender: &WorkerSender,
) -> Option<SessionRegistration> {
    let metadata_sender = sender.clone();
    let media_properties_changed_token = session
        .MediaPropertiesChanged(&TypedEventHandler::<
            GlobalSystemMediaTransportControlsSession,
            MediaPropertiesChangedEventArgs,
        >::new(move |_, _| {
            metadata_sender.send_event(WorkerEvent::MediaProperties {
                session_id,
                observed_at: Instant::now(),
            });
            Ok(())
        }))
        .ok()?;
    let playback_sender = sender.clone();
    let playback_info_changed_token = match session.PlaybackInfoChanged(&TypedEventHandler::<
        GlobalSystemMediaTransportControlsSession,
        PlaybackInfoChangedEventArgs,
    >::new(move |_, _| {
        playback_sender.send_event(WorkerEvent::PlaybackInfo(session_id));
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
            timeline_sender.send_event(WorkerEvent::TimelineProperties(session_id));
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

/// 播放器发布的文本元数据；不含封面，用于廉价判断曲目内容是否变化。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MediaMetadataText {
    pub(super) title: String,
    pub(super) artist: String,
    pub(super) album_artist: String,
    pub(super) subtitle: String,
}

impl MediaMetadataText {
    /// 从完整元数据提取文本键，忽略封面本身。
    pub(super) fn from_metadata(metadata: &MediaMetadata) -> Self {
        Self {
            title: metadata.title.clone(),
            artist: metadata.artist.clone(),
            album_artist: metadata.album_artist.clone(),
            subtitle: metadata.subtitle.clone(),
        }
    }
}

/// SMTC 属性的一次性读取结果：文本立即可用，封面只保留引用供按需解码。
pub(super) struct MediaPropertiesSnapshot {
    pub(super) text: MediaMetadataText,
    pub(super) thumbnail: Option<IRandomAccessStreamReference>,
}

/// 读取文本属性与封面引用；封面数据本身的解码与 Base64 编码由调用方按需触发。
pub(super) fn read_properties(
    session: &GlobalSystemMediaTransportControlsSession,
) -> windows::core::Result<MediaPropertiesSnapshot> {
    let properties = session.TryGetMediaPropertiesAsync()?.get()?;
    Ok(MediaPropertiesSnapshot {
        text: MediaMetadataText {
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
        },
        thumbnail: properties.Thumbnail().ok(),
    })
}

/// 读取播放器实际向 SMTC 发布的全部歌曲元数据。
pub(super) fn read_metadata(
    session: &GlobalSystemMediaTransportControlsSession,
) -> windows::core::Result<MediaMetadata> {
    let properties = read_properties(session)?;
    let thumbnail_data_url = properties
        .thumbnail
        .as_ref()
        .and_then(|thumbnail| read_thumbnail_data_url(thumbnail).ok().flatten());

    Ok(MediaMetadata {
        title: properties.text.title,
        artist: properties.text.artist,
        album_artist: properties.text.album_artist,
        subtitle: properties.text.subtitle,
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
) -> Result<bool, Error> {
    let session = &registration
        .ok_or_else(|| Error::Message("当前没有可控制的 Windows 媒体会话".to_owned()))?
        .session;
    let playback = session.GetPlaybackInfo()?;
    let controls = playback.Controls()?;
    let operation = match action {
        MediaControlAction::TogglePlayPause => {
            if controls.IsPlayPauseToggleEnabled()? {
                session.TryTogglePlayPauseAsync()
            } else if playback.PlaybackStatus()?
                == windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing
            {
                if controls.IsPauseEnabled()? {
                    session.TryPauseAsync()
                } else {
                    return Ok(false);
                }
            } else if controls.IsPlayEnabled()? {
                session.TryPlayAsync()
            } else {
                return Ok(false);
            }
        }
        MediaControlAction::SkipNext => {
            if controls.IsNextEnabled()? {
                session.TrySkipNextAsync()
            } else {
                return Ok(false);
            }
        }
        MediaControlAction::SkipPrevious => {
            if controls.IsPreviousEnabled()? {
                session.TrySkipPreviousAsync()
            } else {
                return Ok(false);
            }
        }
    }?;
    Ok(operation.get()?)
}
