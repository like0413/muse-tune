//! 使用 Windows Core Audio 控制默认播放设备的系统主音量。

use windows::{
    Win32::{
        Foundation::PROPERTYKEY,
        Media::Audio::{
            AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, EDataFlow, ERole,
            Endpoints::{
                IAudioEndpointVolume, IAudioEndpointVolumeCallback,
                IAudioEndpointVolumeCallback_Impl,
            },
            IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
            MMDeviceEnumerator, eMultimedia, eRender,
        },
        System::Com::{CLSCTX_ALL, CoCreateInstance},
    },
    core::{GUID, PCWSTR, implement},
};

use super::{
    MediaVolumeSnapshot,
    monitor::{metrics::WorkerSender, pending_events::WorkerEvent},
};
use crate::error::Error;

const SYSTEM_VOLUME_EVENT_CONTEXT: GUID = GUID::from_u128(0x70f53af2_6efe_4870_985b_937b6d39dc68);

/// 默认多媒体播放设备及其端点音量通知注册。
pub(super) struct SystemVolumeController {
    enumerator: IMMDeviceEnumerator,
    device_notification: IMMNotificationClient,
    endpoint: Option<EndpointVolumeRegistration>,
    sender: WorkerSender,
}

impl SystemVolumeController {
    /// 订阅默认设备变化，并绑定当前默认多媒体播放设备。
    pub(super) fn new(sender: WorkerSender) -> Result<Self, Error> {
        // SAFETY: 媒体 worker 已初始化 MTA；枚举器及其注册只由该线程持有和释放。
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
        let device_notification: IMMNotificationClient = DeviceNotification {
            sender: sender.clone(),
        }
        .into();
        // SAFETY: 回调对象与枚举器都由控制器持有，Drop 时使用同一实例注销。
        unsafe { enumerator.RegisterEndpointNotificationCallback(&device_notification) }?;
        let mut controller = Self {
            enumerator,
            device_notification,
            endpoint: None,
            sender,
        };
        controller.rebind_default_endpoint();
        Ok(controller)
    }

    /// 默认设备变化后重新建立端点音量回调；当前没有输出设备时保留空状态。
    pub(super) fn rebind_default_endpoint(&mut self) {
        self.endpoint = None;
        match EndpointVolumeRegistration::new(&self.enumerator, self.sender.clone()) {
            Ok(registration) => self.endpoint = Some(registration),
            Err(error) => log::warn!("绑定 Windows 默认播放设备主音量失败: {error}"),
        }
    }

    /// 返回系统主音量与独立静音状态。
    pub(super) fn snapshot(&self) -> Option<MediaVolumeSnapshot> {
        self.endpoint
            .as_ref()
            .and_then(|endpoint| endpoint.snapshot())
    }

    /// 设置系统主音量；滑块操作同时解除静音。
    pub(super) fn set_level(&self, level: f32) -> Result<MediaVolumeSnapshot, Error> {
        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or_else(|| Error::Message("Windows 默认播放设备当前不可用".to_owned()))?;
        endpoint.set_level(level)
    }

    /// 切换系统主音量的原生静音状态。
    pub(super) fn toggle_muted(&self) -> Result<MediaVolumeSnapshot, Error> {
        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or_else(|| Error::Message("Windows 默认播放设备当前不可用".to_owned()))?;
        endpoint.toggle_muted()
    }
}

impl Drop for SystemVolumeController {
    fn drop(&mut self) {
        self.endpoint = None;
        // SAFETY: device_notification 正是向该 enumerator 注册的同一 COM 实例。
        let _ = unsafe {
            self.enumerator
                .UnregisterEndpointNotificationCallback(&self.device_notification)
        };
    }
}

/// 保持默认端点音量接口及其回调存活。
struct EndpointVolumeRegistration {
    volume: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

impl EndpointVolumeRegistration {
    fn new(enumerator: &IMMDeviceEnumerator, sender: WorkerSender) -> windows::core::Result<Self> {
        // SAFETY: 只读取默认多媒体播放端点，并在同一个 MTA worker 上激活音量接口。
        let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) }?;
        let volume = unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) }?;
        let callback: IAudioEndpointVolumeCallback = EndpointVolumeNotification { sender }.into();
        // SAFETY: callback 由返回的注册结构持有，并在 Drop 时成对注销。
        unsafe { volume.RegisterControlChangeNotify(&callback) }?;
        Ok(Self { volume, callback })
    }

    fn snapshot(&self) -> Option<MediaVolumeSnapshot> {
        // SAFETY: 两个读取都作用于本线程持有的同一端点接口；任一失败即不发布半份快照。
        Some(MediaVolumeSnapshot {
            level: unsafe { self.volume.GetMasterVolumeLevelScalar() }
                .ok()?
                .clamp(0.0, 1.0),
            muted: unsafe { self.volume.GetMute() }.ok()?.as_bool(),
        })
    }

    fn set_level(&self, level: f32) -> Result<MediaVolumeSnapshot, Error> {
        let level = level.clamp(0.0, 1.0);
        // SAFETY: 两次写入作用于同一默认端点，并用同一上下文标识本应用发起的变更。
        unsafe {
            self.volume
                .SetMasterVolumeLevelScalar(level, &SYSTEM_VOLUME_EVENT_CONTEXT)
                .and_then(|()| self.volume.SetMute(false, &SYSTEM_VOLUME_EVENT_CONTEXT))
        }?;
        Ok(MediaVolumeSnapshot {
            level,
            muted: false,
        })
    }

    fn toggle_muted(&self) -> Result<MediaVolumeSnapshot, Error> {
        let current = self
            .snapshot()
            .ok_or_else(|| Error::Message("读取 Windows 系统主音量失败".to_owned()))?;
        let muted = !current.muted;
        // SAFETY: 写入作用于本线程持有的默认端点接口。
        unsafe { self.volume.SetMute(muted, &SYSTEM_VOLUME_EVENT_CONTEXT) }?;
        Ok(MediaVolumeSnapshot { muted, ..current })
    }
}

impl Drop for EndpointVolumeRegistration {
    fn drop(&mut self) {
        // SAFETY: callback 正是向该 volume 注册的同一 COM 实例。
        let _ = unsafe { self.volume.UnregisterControlChangeNotify(&self.callback) };
    }
}

#[implement(IAudioEndpointVolumeCallback)]
struct EndpointVolumeNotification {
    sender: WorkerSender,
}

impl IAudioEndpointVolumeCallback_Impl for EndpointVolumeNotification_Impl {
    fn OnNotify(
        &self,
        _notification: *mut AUDIO_VOLUME_NOTIFICATION_DATA,
    ) -> windows::core::Result<()> {
        self.sender.send_event(WorkerEvent::SystemVolume);
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DeviceNotification {
    sender: WorkerSender,
}

impl IMMNotificationClient_Impl for DeviceNotification_Impl {
    fn OnDeviceStateChanged(
        &self,
        _device_id: &PCWSTR,
        _new_state: DEVICE_STATE,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnDeviceAdded(&self, _device_id: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnDeviceRemoved(&self, _device_id: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _device_id: &PCWSTR,
    ) -> windows::core::Result<()> {
        if flow == eRender && role == eMultimedia {
            self.sender.send_event(WorkerEvent::DefaultAudioEndpoint);
        }
        Ok(())
    }

    fn OnPropertyValueChanged(
        &self,
        _device_id: &PCWSTR,
        _key: &PROPERTYKEY,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}
