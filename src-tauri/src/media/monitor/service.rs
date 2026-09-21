use std::{
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use tauri::{AppHandle, Runtime};

use super::{WorkerMessage, metrics, metrics::WorkerSender, worker::run_worker};
use crate::error::Error;
use crate::media::{
    MediaControlAction, MediaRuntimeDiagnostics, MediaSessionSelectionPolicy, MediaSessionSnapshot,
    MediaSnapshotDiagnostics, MediaSnapshotSubscriber, MediaVolumeSnapshot,
};

/// 常规请求（控制、播放器窗口、音量、频谱、策略）的等待上限：超时只把该次调用变成文本错误，
/// 不向 worker 发任何取消消息——worker 可能只是在处理慢事件或正被 WinRT 阻塞，仍会继续服务后续请求。
const WORKER_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
/// 诊断查询的等待上限：它由设置页按需拉取，宁可快速失败也不让界面一直等采集结果，因此只给 1 秒；
/// 同样只影响这一次调用，worker 不受影响。
const DIAGNOSTICS_RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);

/// 面向 Tauri command 的线程安全媒体服务句柄。
#[derive(Clone)]
pub struct MediaService {
    inner: Arc<MediaServiceInner>,
}

struct MediaServiceInner {
    sender: WorkerSender,
    snapshot: Arc<RwLock<Option<MediaSessionSnapshot>>>,
    shutdown_requested: AtomicBool,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl MediaService {
    /// 启动独立 WinRT MTA 线程，避免媒体 API 阻塞 Tauri 主线程。
    pub(crate) fn initialize<R: Runtime>(
        app: AppHandle<R>,
        snapshot_subscriber: MediaSnapshotSubscriber,
    ) -> Result<Self, std::io::Error> {
        let (sender, receiver, worker_metrics) = metrics::channel();
        let snapshot = Arc::new(RwLock::new(None));
        let worker_sender = sender.clone();
        let worker_snapshot = Arc::clone(&snapshot);

        let worker = thread::Builder::new()
            .name("media-session-monitor".to_owned())
            .spawn(move || {
                run_worker(
                    app,
                    worker_sender,
                    receiver,
                    worker_metrics,
                    worker_snapshot,
                    snapshot_subscriber,
                );
            })?;

        Ok(Self {
            inner: Arc::new(MediaServiceInner {
                sender,
                snapshot,
                shutdown_requested: AtomicBool::new(false),
                worker: Mutex::new(Some(worker)),
            }),
        })
    }

    /// 非阻塞请求媒体 worker 退出，供 Tauri `ExitRequested` 提前释放回调源。
    pub(crate) fn request_shutdown(&self) {
        self.inner.request_shutdown();
    }

    /// 等待媒体 worker 完成事件注销、频谱停止和 WinRT 反初始化。
    pub(crate) fn shutdown(&self) {
        self.inner.shutdown();
    }

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
    pub(crate) fn runtime_diagnostics(&self) -> Result<MediaRuntimeDiagnostics, Error> {
        self.request(
            WorkerMessage::GetDiagnostics,
            DIAGNOSTICS_RESPONSE_TIMEOUT,
            "媒体会话未返回诊断状态",
        )
    }

    /// 将控制请求串行投递给持有当前 WinRT 会话的线程。
    pub fn control(&self, action: MediaControlAction) -> Result<bool, Error> {
        self.request(
            |sender| WorkerMessage::Control(action, sender),
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回控制结果",
        )?
    }

    /// 开关当前选中会话所属的播放器主窗口：已在前台时关闭，最小化或隐藏时还原或显示。
    pub fn toggle_player_window(&self) -> Result<(), Error> {
        self.request(
            WorkerMessage::TogglePlayerWindow,
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回播放器窗口开关结果",
        )?
    }

    /// 更新多播放器会话选择策略，并立即重新计算控制目标。
    pub fn set_selection_policy(&self, policy: MediaSessionSelectionPolicy) -> Result<(), Error> {
        self.request(
            |sender| WorkerMessage::SelectionPolicyChanged(policy, sender),
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回策略更新结果",
        )?
    }

    /// 返回当前播放器的 Windows 单应用音量。
    pub fn volume(&self) -> Result<Option<MediaVolumeSnapshot>, Error> {
        self.request(
            WorkerMessage::GetVolume,
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回应用音量",
        )
    }

    /// 设置当前播放器的 Windows 单应用音量。
    pub fn set_volume(&self, level: f32) -> Result<MediaVolumeSnapshot, Error> {
        self.request(
            |sender| WorkerMessage::SetVolume(level, sender),
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回音量设置结果",
        )?
    }

    /// 切换当前播放器的 Windows 单应用静音状态。
    pub fn toggle_mute(&self) -> Result<MediaVolumeSnapshot, Error> {
        self.request(
            WorkerMessage::ToggleMute,
            WORKER_RESPONSE_TIMEOUT,
            "媒体会话未返回静音切换结果",
        )?
    }

    /// 启用或停止当前播放器的真实音频频谱采集。
    pub fn set_spectrum_enabled(&self, enabled: bool, frame_rate: u16) -> Result<(), Error> {
        self.request(
            |sender| WorkerMessage::SpectrumEnabled(enabled, frame_rate, sender),
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
    ) -> Result<T, Error> {
        let (result_sender, result_receiver) = mpsc::sync_channel(1);
        self.inner
            .sender
            .send(message(result_sender))
            .map_err(|_| Error::Message("媒体会话监控线程不可用".to_owned()))?;
        result_receiver
            .recv_timeout(timeout)
            .map_err(|_| Error::Message(timeout_message.to_owned()))
    }
}

impl MediaServiceInner {
    /// 保证无论收到多少次退出事件，都只向 worker 投递一次终止消息。
    fn request_shutdown(&self) {
        if !self.shutdown_requested.swap(true, Ordering::AcqRel) {
            let _ = self.sender.send(WorkerMessage::Shutdown);
        }
    }

    /// 回收唯一 worker；重复调用保持幂等，且不会错误等待当前线程自身。
    fn shutdown(&self) {
        self.request_shutdown();
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(worker) = worker {
            if worker.thread().id() == thread::current().id() {
                log::warn!("媒体 worker 尝试等待自身，已跳过 join");
                return;
            }
            if worker.join().is_err() {
                log::warn!("媒体会话监控线程异常退出");
            }
        }
    }
}

impl Drop for MediaServiceInner {
    /// 显式退出编排遗漏时仍尝试停止并回收唯一 worker。
    fn drop(&mut self) {
        self.shutdown();
    }
}
