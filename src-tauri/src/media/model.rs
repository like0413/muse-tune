use windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus;

/// 当前 GSMTC 会话的媒体信息与控制状态。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSessionSnapshot {
    pub source_icon_data_url: Option<String>,
    pub player: MediaPlayer,
    pub metadata: MediaMetadata,
    pub playback: MediaPlayback,
}

/// 已接入播放器及无法识别的通用 SMTC 会话。
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum MediaPlayer {
    QqMusic,
    NeteaseCloudMusic,
    SodaMusic,
    KugouMusic,
    #[default]
    Other,
}

/// 多播放器同时存在时的会话选择方式。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaSessionSelectionStrategy {
    FollowWindows,
    #[default]
    RecentPlayback,
    StickyCurrent,
    FixedPriority,
}

/// 会话选择策略及固定优先级的完整配置。
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSessionSelectionPolicy {
    pub strategy: MediaSessionSelectionStrategy,
    pub player_priority: Vec<MediaPlayer>,
}

impl Default for MediaSessionSelectionPolicy {
    fn default() -> Self {
        Self {
            strategy: MediaSessionSelectionStrategy::RecentPlayback,
            player_priority: vec![
                MediaPlayer::QqMusic,
                MediaPlayer::NeteaseCloudMusic,
                MediaPlayer::SodaMusic,
                MediaPlayer::KugouMusic,
            ],
        }
    }
}

/// 播放器通过 SMTC 主动发布的歌曲元数据。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadata {
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub subtitle: String,
    pub thumbnail_data_url: Option<String>,
}

/// 与当前阶段播放按钮有关的 SMTC 状态和能力开关。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPlayback {
    pub status: MediaPlaybackStatus,
    pub controls: MediaPlaybackControls,
}

/// 播放器当前明确声明支持的基础播放控制。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPlaybackControls {
    pub can_play: bool,
    pub can_pause: bool,
    pub can_toggle_play_pause: bool,
    pub can_skip_next: bool,
    pub can_skip_previous: bool,
}

/// GSMTC 的六种标准播放状态。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaPlaybackStatus {
    Closed,
    Opened,
    Changing,
    Stopped,
    Playing,
    Paused,
    #[default]
    Unknown,
}

/// 前端允许发送的基础媒体控制动作。
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaControlAction {
    TogglePlayPause,
    SkipNext,
    SkipPrevious,
}

impl From<GlobalSystemMediaTransportControlsSessionPlaybackStatus> for MediaPlaybackStatus {
    fn from(status: GlobalSystemMediaTransportControlsSessionPlaybackStatus) -> Self {
        match status {
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Closed => Self::Closed,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Opened => Self::Opened,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Changing => Self::Changing,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped => Self::Stopped,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => Self::Playing,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => Self::Paused,
            _ => Self::Unknown,
        }
    }
}
