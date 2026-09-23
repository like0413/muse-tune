//! 通过 Windows GSMTC 事件监控媒体会话。
//!
//! 模块根只持有 worker 的共享状态定义与消息契约，具体职责分布在子模块里：
//! - `worker`：事件循环本体，独占 WinRT MTA 线程；
//! - `registry`：GSMTC 会话注册表的同步与增删；
//! - `playback`：单会话元数据、播放状态与时间线刷新；
//! - `selection`：会话择优与切歌保持窗口；
//! - `volume_binding`：选中会话到 Windows 应用音频会话的绑定与退避重试；
//! - `session` / `publisher` / `metrics` / `deadlines` / `pending_events` / `apartment`：平台订阅、广播、指标与定时。

use std::{sync::mpsc, time::Instant};

use tauri::Runtime;

use super::{
    MediaControlAction, MediaRuntimeDiagnostics, MediaSessionSelectionPolicy, MediaSessionSnapshot,
    MediaVolumeSnapshot, spectrum::AudioSpectrumController, volume::ApplicationVolumeController,
};
use crate::error::Error;

mod apartment;
mod deadlines;
pub(super) mod metrics;
pub(in crate::media) mod pending_events;
mod playback;
mod publisher;
mod registry;
mod selection;
mod service;
mod session;
mod volume_binding;
mod worker;

pub use service::MediaService;
use session::SessionRegistration;

/// worker 线程接收的前台请求；带响应通道的消息必须逐一回复。
pub(super) enum WorkerMessage {
    EventsReady,
    SelectionPolicyChanged(
        MediaSessionSelectionPolicy,
        mpsc::SyncSender<Result<(), Error>>,
    ),
    Control(MediaControlAction, mpsc::SyncSender<Result<bool, Error>>),
    TogglePlayerWindow(mpsc::SyncSender<Result<(), Error>>),
    GetVolume(mpsc::SyncSender<Option<MediaVolumeSnapshot>>),
    SetVolume(f32, mpsc::SyncSender<Result<MediaVolumeSnapshot, Error>>),
    ToggleMute(mpsc::SyncSender<Result<MediaVolumeSnapshot, Error>>),
    GetSystemVolume(mpsc::SyncSender<Option<MediaVolumeSnapshot>>),
    SetSystemVolume(f32, mpsc::SyncSender<Result<MediaVolumeSnapshot, Error>>),
    ToggleSystemMute(mpsc::SyncSender<Result<MediaVolumeSnapshot, Error>>),
    GetDiagnostics(mpsc::SyncSender<MediaRuntimeDiagnostics>),
    SpectrumEnabled(bool, u16, mpsc::SyncSender<Result<(), Error>>),
    Shutdown,
}

/// 单个 GSMTC 会话的事件注册、快照与最近播放序号。
struct SessionEntry {
    id: u64,
    registration: SessionRegistration,
    snapshot: MediaSessionSnapshot,
    /// 当前展示的封面所对应的文本元数据键，用于跳过未变内容的封面重复解码。
    ///
    /// 只在读到“与展示中的封面不同”的图时才记录：文本键本身证明不了封面归属，
    /// 切歌瞬间读到的可能还是上一首的图（见 `playback::is_new_thumbnail`）。
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
