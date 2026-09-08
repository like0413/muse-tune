//! 使用 Windows Core Audio 控制当前播放器的单应用音量，绝不修改系统端点音量。

use std::{collections::HashSet, sync::mpsc::Sender};

use windows::{
    Win32::{
        Media::Audio::{
            AudioSessionDisconnectReason, AudioSessionState, AudioSessionStateActive,
            DEVICE_STATE_ACTIVE, IAudioSessionControl, IAudioSessionControl2, IAudioSessionEvents,
            IAudioSessionEvents_Impl, IAudioSessionManager2, IAudioSessionNotification,
            IAudioSessionNotification_Impl, IMMDeviceEnumerator, ISimpleAudioVolume,
            MMDeviceEnumerator, eRender,
        },
        System::Com::{CLSCTX_ALL, CoCreateInstance},
    },
    core::{BOOL, GUID, Interface, PCWSTR, Ref, implement},
};

use super::{MediaVolumeSnapshot, monitor::WorkerMessage, process::find_process_ids};

const VOLUME_EVENT_CONTEXT: GUID = GUID::from_u128(0x16c5b57c_2d31_45a5_9c9e_f97a63d20f31);

/// 当前播放器跨输出设备的全部应用音频会话。
pub(super) struct ApplicationVolumeController {
    target_id: Option<u64>,
    device_registrations: Vec<DeviceRegistration>,
    session_registrations: Vec<VolumeSessionRegistration>,
    sender: Sender<WorkerMessage>,
}

impl ApplicationVolumeController {
    /// 创建尚未绑定播放器的控制器。
    pub(super) fn new(sender: Sender<WorkerMessage>) -> Self {
        Self {
            target_id: None,
            device_registrations: Vec::new(),
            session_registrations: Vec::new(),
            sender,
        }
    }

    /// 绑定已选媒体会话对应的全部进程与音频输出设备。
    pub(super) fn bind(
        &mut self,
        target_id: Option<u64>,
        source_app_id: &str,
        executable_names: &[&str],
    ) {
        self.target_id = target_id;
        self.session_registrations.clear();
        self.device_registrations.clear();

        let Some(target_id) = target_id else {
            return;
        };
        let process_ids = find_process_ids(source_app_id, executable_names);
        if process_ids.is_empty() {
            log::debug!("未找到当前播放器进程，暂不绑定应用音量");
        }
        if let Err(error) = self.bind_devices(target_id, &process_ids) {
            log::warn!("绑定播放器应用音量失败: {error}");
        }
    }

    /// 音频会话创建或失效后，按当前播放器重新枚举一次。
    pub(super) fn rebind(
        &mut self,
        target_id: u64,
        source_app_id: &str,
        executable_names: &[&str],
    ) {
        if self.target_id == Some(target_id) {
            self.bind(Some(target_id), source_app_id, executable_names);
        }
    }

    /// 返回活动会话优先的当前应用音量。
    pub(super) fn snapshot(&self) -> Option<MediaVolumeSnapshot> {
        self.session_registrations
            .iter()
            .find(|registration| registration.is_active())
            .or_else(|| self.session_registrations.first())
            .and_then(VolumeSessionRegistration::snapshot)
    }

    /// 同步设置该播放器的全部音频会话；拖动音量时自动解除静音。
    pub(super) fn set_level(&self, level: f32) -> Result<MediaVolumeSnapshot, String> {
        if self.session_registrations.is_empty() {
            return Err("当前播放器尚未创建 Windows 应用音频会话".to_owned());
        }
        let level = level.clamp(0.0, 1.0);
        for registration in &self.session_registrations {
            registration.set_level(level)?;
        }
        Ok(MediaVolumeSnapshot {
            level,
            muted: false,
        })
    }

    /// 切换当前播放器全部音频会话的原生静音状态。
    pub(super) fn toggle_muted(&self) -> Result<MediaVolumeSnapshot, String> {
        let current = self
            .snapshot()
            .ok_or_else(|| "当前播放器尚未创建 Windows 应用音频会话".to_owned())?;
        let muted = !current.muted;
        for registration in &self.session_registrations {
            registration.set_muted(muted)?;
        }
        Ok(MediaVolumeSnapshot { muted, ..current })
    }

    /// 枚举全部活动输出端点，同时订阅新音频会话事件。
    fn bind_devices(
        &mut self,
        target_id: u64,
        process_ids: &HashSet<u32>,
    ) -> windows::core::Result<()> {
        // SAFETY: 媒体工作线程已初始化为 MTA，COM 对象只在该线程创建和释放。
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
        // SAFETY: 枚举只读取当前活动的渲染端点。
        let devices = unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }?;
        for index in 0..unsafe { devices.GetCount() }? {
            let Ok(device) = (unsafe { devices.Item(index) }) else {
                continue;
            };
            let Ok(manager) =
                (unsafe { device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) })
            else {
                continue;
            };
            let Ok(session_enumerator) = (unsafe { manager.GetSessionEnumerator() }) else {
                continue;
            };

            // Microsoft 要求先创建会话枚举器，再注册新会话通知。
            let notification: IAudioSessionNotification = SessionNotification {
                sender: self.sender.clone(),
                target_id,
            }
            .into();
            if let Err(error) = unsafe { manager.RegisterSessionNotification(&notification) } {
                log::warn!("订阅音频设备新会话通知失败: {error}");
                continue;
            }
            self.device_registrations.push(DeviceRegistration {
                manager,
                notification,
            });

            let session_count = unsafe { session_enumerator.GetCount() }.unwrap_or_default();
            for session_index in 0..session_count {
                let Ok(control) = (unsafe { session_enumerator.GetSession(session_index) }) else {
                    continue;
                };
                let Ok(control2) = control.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                let Ok(process_id) = (unsafe { control2.GetProcessId() }) else {
                    continue;
                };
                if process_ids.contains(&process_id) {
                    match VolumeSessionRegistration::new(
                        control,
                        process_id,
                        self.sender.clone(),
                        target_id,
                    ) {
                        Ok(registration) => self.session_registrations.push(registration),
                        Err(error) => log::warn!("订阅播放器音量会话失败: {error}"),
                    }
                }
            }
        }
        Ok(())
    }

    /// 返回活动音频会话对应的进程，供按进程回环捕获复用同一目标。
    pub(super) fn capture_process_id(&self) -> Option<u32> {
        self.session_registrations
            .iter()
            .find(|registration| registration.is_active())
            .or_else(|| self.session_registrations.first())
            .map(|registration| registration.process_id)
    }
}

/// 保持设备级新会话通知存活，并在释放时成对注销。
struct DeviceRegistration {
    manager: IAudioSessionManager2,
    notification: IAudioSessionNotification,
}

impl Drop for DeviceRegistration {
    fn drop(&mut self) {
        // SAFETY: notification 正是向该 manager 注册的同一 COM 实例。
        let _ = unsafe {
            self.manager
                .UnregisterSessionNotification(&self.notification)
        };
    }
}

/// 保持单个应用音频会话及其事件回调存活。
struct VolumeSessionRegistration {
    control: IAudioSessionControl,
    process_id: u32,
    volume: ISimpleAudioVolume,
    events: IAudioSessionEvents,
}

impl VolumeSessionRegistration {
    /// 为匹配到的音频会话订阅音量、状态与断开事件。
    fn new(
        control: IAudioSessionControl,
        process_id: u32,
        sender: Sender<WorkerMessage>,
        target_id: u64,
    ) -> windows::core::Result<Self> {
        let volume: ISimpleAudioVolume = control.cast()?;
        let events: IAudioSessionEvents = VolumeSessionEvents { sender, target_id }.into();
        // SAFETY: 回调对象由注册结构持有，生命周期覆盖注册期。
        unsafe { control.RegisterAudioSessionNotification(&events) }?;
        Ok(Self {
            control,
            process_id,
            volume,
            events,
        })
    }

    /// 读取当前会话的标量音量与静音状态。
    fn snapshot(&self) -> Option<MediaVolumeSnapshot> {
        Some(MediaVolumeSnapshot {
            level: unsafe { self.volume.GetMasterVolume() }
                .ok()?
                .clamp(0.0, 1.0),
            muted: unsafe { self.volume.GetMute() }.ok()?.as_bool(),
        })
    }

    /// 判断此音频会话是否正在产生或准备产生声音。
    fn is_active(&self) -> bool {
        unsafe { self.control.GetState() }.is_ok_and(|state| state == AudioSessionStateActive)
    }

    /// 设置单会话音量，并确保滑块操作能够从静音恢复。
    fn set_level(&self, level: f32) -> Result<(), String> {
        unsafe {
            self.volume
                .SetMasterVolume(level, &VOLUME_EVENT_CONTEXT)
                .and_then(|()| self.volume.SetMute(false, &VOLUME_EVENT_CONTEXT))
        }
        .map_err(|error| error.to_string())
    }

    /// 使用 Core Audio 会话静音接口修改单个播放器音频会话。
    fn set_muted(&self, muted: bool) -> Result<(), String> {
        unsafe { self.volume.SetMute(muted, &VOLUME_EVENT_CONTEXT) }
            .map_err(|error| error.to_string())
    }
}

impl Drop for VolumeSessionRegistration {
    fn drop(&mut self) {
        // SAFETY: events 正是向该 control 注册的同一 COM 实例。
        let _ = unsafe {
            self.control
                .UnregisterAudioSessionNotification(&self.events)
        };
    }
}

/// 把 Core Audio 回调压缩为媒体工作线程消息，避免在系统回调线程执行 IPC。
#[implement(IAudioSessionEvents)]
struct VolumeSessionEvents {
    sender: Sender<WorkerMessage>,
    target_id: u64,
}

#[allow(non_snake_case)]
impl IAudioSessionEvents_Impl for VolumeSessionEvents_Impl {
    fn OnSimpleVolumeChanged(
        &self,
        _newvolume: f32,
        _newmute: BOOL,
        _eventcontext: *const GUID,
    ) -> windows::core::Result<()> {
        let _ = self
            .sender
            .send(WorkerMessage::VolumeChanged(self.target_id));
        Ok(())
    }

    fn OnStateChanged(&self, _newstate: AudioSessionState) -> windows::core::Result<()> {
        let _ = self
            .sender
            .send(WorkerMessage::VolumeChanged(self.target_id));
        Ok(())
    }

    fn OnSessionDisconnected(
        &self,
        _disconnectreason: AudioSessionDisconnectReason,
    ) -> windows::core::Result<()> {
        let _ = self
            .sender
            .send(WorkerMessage::VolumeSessionsChanged(self.target_id));
        Ok(())
    }

    fn OnDisplayNameChanged(
        &self,
        _newdisplayname: &PCWSTR,
        _eventcontext: *const GUID,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnIconPathChanged(
        &self,
        _newiconpath: &PCWSTR,
        _eventcontext: *const GUID,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnChannelVolumeChanged(
        &self,
        _channelcount: u32,
        _newchannelvolumearray: *const f32,
        _changedchannel: u32,
        _eventcontext: *const GUID,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnGroupingParamChanged(
        &self,
        _newgroupingparam: *const GUID,
        _eventcontext: *const GUID,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

/// 新会话出现时通知工作线程重新匹配 PID；不在回调线程枚举进程。
#[implement(IAudioSessionNotification)]
struct SessionNotification {
    sender: Sender<WorkerMessage>,
    target_id: u64,
}

#[allow(non_snake_case)]
impl IAudioSessionNotification_Impl for SessionNotification_Impl {
    fn OnSessionCreated(
        &self,
        _newsession: Ref<'_, IAudioSessionControl>,
    ) -> windows::core::Result<()> {
        let _ = self
            .sender
            .send(WorkerMessage::VolumeSessionsChanged(self.target_id));
        Ok(())
    }
}
