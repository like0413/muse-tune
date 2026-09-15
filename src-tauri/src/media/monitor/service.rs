use std::{
    sync::{
        Arc, RwLock,
        mpsc::{self, Sender},
    },
    thread,
    time::Duration,
};

use tauri::{AppHandle, Runtime};

use super::{MediaSnapshotSubscriber, WorkerMessage, run_worker};
use crate::media::{
    MediaControlAction, MediaRuntimeDiagnostics, MediaSessionSelectionPolicy, MediaSessionSnapshot,
    MediaSnapshotDiagnostics, MediaVolumeSnapshot,
};

const WORKER_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
const DIAGNOSTICS_RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);

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
    pub(crate) fn initialize<R: Runtime>(
        app: AppHandle<R>,
        snapshot_subscriber: MediaSnapshotSubscriber,
    ) -> Result<Self, std::io::Error> {
        let (sender, receiver) = mpsc::channel();
        let snapshot = Arc::new(RwLock::new(None));
        let worker_sender = sender.clone();
        let worker_snapshot = Arc::clone(&snapshot);

        thread::Builder::new()
            .name("media-session-monitor".to_owned())
            .spawn(move || {
                run_worker(
                    app,
                    worker_sender,
                    receiver,
                    worker_snapshot,
                    snapshot_subscriber,
                );
            })?;

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
    pub(crate) fn diagnostics_snapshot(&self) -> Option<MediaSnapshotDiagnostics> {
        self.inner.snapshot.read().ok().and_then(|snapshot| {
            snapshot.as_ref().map(|snapshot| MediaSnapshotDiagnostics {
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
    pub(crate) fn runtime_diagnostics(&self) -> Result<MediaRuntimeDiagnostics, String> {
        self.request(
            WorkerMessage::GetDiagnostics,
            DIAGNOSTICS_RESPONSE_TIMEOUT,
            "媒体会话未返回诊断状态",
        )
    }

    /// 将控制请求串行投递给持有当前 WinRT 会话的线程。
    pub fn control(&self, action: MediaControlAction) -> Result<bool, String> {
        self.request(
            |sender| WorkerMessage::Control(action, sender),
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回控制结果",
        )?
    }

    /// 更新多播放器会话选择策略，并立即重新计算控制目标。
    pub fn set_selection_policy(&self, policy: MediaSessionSelectionPolicy) -> Result<(), String> {
        self.request(
            |sender| WorkerMessage::SelectionPolicyChanged(policy, sender),
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回策略更新结果",
        )?
    }

    /// 返回当前播放器的 Windows 单应用音量。
    pub fn volume(&self) -> Result<Option<MediaVolumeSnapshot>, String> {
        self.request(
            WorkerMessage::GetVolume,
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回应用音量",
        )
    }

    /// 设置当前播放器的 Windows 单应用音量。
    pub fn set_volume(&self, level: f32) -> Result<MediaVolumeSnapshot, String> {
        self.request(
            |sender| WorkerMessage::SetVolume(level, sender),
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回音量设置结果",
        )?
    }

    /// 切换当前播放器的 Windows 单应用静音状态。
    pub fn toggle_mute(&self) -> Result<MediaVolumeSnapshot, String> {
        self.request(
            WorkerMessage::ToggleMute,
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回静音切换结果",
        )?
    }

    /// 启用或停止当前播放器的真实音频频谱采集。
    pub fn set_spectrum_enabled(&self, enabled: bool) -> Result<(), String> {
        self.request(
            |sender| WorkerMessage::SpectrumEnabled(enabled, sender),
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回频谱开关结果",
        )?
    }

    /// 统一执行带单次响应通道的 worker 请求，保持门面超时和错误语义一致。
    fn request<T>(
        &self,
        message: impl FnOnce(mpsc::SyncSender<T>) -> WorkerMessage,
        timeout: Duration,
        timeout_message: &str,
    ) -> Result<T, String> {
        let (result_sender, result_receiver) = mpsc::sync_channel(1);
        self.inner
            .sender
            .send(message(result_sender))
            .map_err(|_| "媒体会话监控线程不可用".to_owned())?;
        result_receiver
            .recv_timeout(timeout)
            .map_err(|_| timeout_message.to_owned())
    }
}

impl Drop for MediaServiceInner {
    /// 最后一个服务句柄释放时请求唯一 worker 正常退出。
    fn drop(&mut self) {
        let _ = self.sender.send(WorkerMessage::Shutdown);
    }
}
