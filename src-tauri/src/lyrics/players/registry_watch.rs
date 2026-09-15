use std::{ffi::c_void, io, iter, sync::Arc, thread};

use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_FAILED, WAIT_OBJECT_0},
        System::{
            Registry::{
                HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, REG_NOTIFY_CHANGE_LAST_SET, RegCloseKey,
                RegNotifyChangeKeyValue, RegOpenKeyExW,
            },
            Threading::{CreateEventW, INFINITE, SetEvent, WaitForMultipleObjects},
        },
    },
    core::PCWSTR,
};

pub(super) type RegistryValueChangeCallback = Arc<dyn Fn() + Send + Sync>;

/// 持有注册表监听线程及其停止事件；析构时先唤醒线程，再同步回收。
pub(in crate::lyrics) struct RegistryWatchHandle {
    stop_event: OwnedEvent,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for RegistryWatchHandle {
    fn drop(&mut self) {
        if let Err(error) = self.stop_event.signal() {
            log::warn!("停止注册表监听线程失败: {error}");
        }
        if self
            .worker
            .take()
            .is_some_and(|worker| worker.join().is_err())
        {
            log::warn!("注册表监听线程异常退出");
        }
    }
}

/// 使用 Windows 异步注册表通知监听当前用户键值变化。
pub(super) fn watch_current_user_value_changes(
    thread_name: &'static str,
    subkey: &str,
    on_change: RegistryValueChangeCallback,
) -> io::Result<RegistryWatchHandle> {
    let key = OwnedRegistryKey::open_current_user(subkey)?;
    let stop_event = OwnedEvent::create()?;
    let stop_handle = stop_event.raw();
    let worker = thread::Builder::new()
        .name(thread_name.to_owned())
        .spawn(move || run_registry_watcher(key, stop_handle, on_change))?;

    Ok(RegistryWatchHandle {
        stop_event,
        worker: Some(worker),
    })
}

/// 注册一次通知并在变更事件与停止事件之间阻塞等待。
fn run_registry_watcher(
    key: OwnedRegistryKey,
    stop_handle: usize,
    on_change: RegistryValueChangeCallback,
) {
    let change_event = match OwnedEvent::create() {
        Ok(event) => event,
        Err(error) => {
            log::warn!("创建注册表变化事件失败: {error}");
            return;
        }
    };
    if let Err(error) = arm_change_notification(&key, &change_event) {
        log::warn!("注册注册表变化通知失败: {error}");
        return;
    }

    let handles = [change_event.handle(), raw_handle(stop_handle)];
    loop {
        // SAFETY: 两个句柄在整个等待期间有效，且数组长度固定为 2。
        let result = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
        if result == WAIT_OBJECT_0 {
            // 先重新注册一次性通知，避免回调执行期间漏掉连续配置变化。
            if let Err(error) = arm_change_notification(&key, &change_event) {
                log::warn!("重新注册注册表变化通知失败: {error}");
                break;
            }
            on_change();
        } else if result.0 == WAIT_OBJECT_0.0 + 1 {
            break;
        } else if result == WAIT_FAILED {
            log::warn!("等待注册表变化事件失败: {}", io::Error::last_os_error());
            break;
        } else {
            log::warn!("等待注册表变化事件返回未知状态: {}", result.0);
            break;
        }
    }
}

/// 将一次性异步注册表通知绑定到变化事件。
fn arm_change_notification(key: &OwnedRegistryKey, event: &OwnedEvent) -> io::Result<()> {
    // SAFETY: key 与 event 均由当前监听生命周期持有，异步通知结束前不会释放。
    let result = unsafe {
        RegNotifyChangeKeyValue(
            key.handle(),
            false,
            REG_NOTIFY_CHANGE_LAST_SET,
            Some(event.handle()),
            true,
        )
    };
    win32_result(result)
}

/// 只持有一个当前用户注册表键，并确保仅关闭一次。
struct OwnedRegistryKey(usize);

impl OwnedRegistryKey {
    /// 以通知权限打开当前用户下的指定注册表键。
    fn open_current_user(subkey: &str) -> io::Result<Self> {
        let wide_subkey = subkey
            .encode_utf16()
            .chain(iter::once(0))
            .collect::<Vec<_>>();
        let mut key = HKEY::default();
        // SAFETY: wide_subkey 以 NUL 结尾且在调用期间有效；输出句柄由返回值接管。
        let result = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR::from_raw(wide_subkey.as_ptr()),
                None,
                KEY_NOTIFY,
                &mut key,
            )
        };
        win32_result(result)?;
        Ok(Self(key.0 as usize))
    }

    /// 重建仅供 Win32 调用使用的非拥有句柄值。
    fn handle(&self) -> HKEY {
        HKEY(self.0 as *mut c_void)
    }
}

impl Drop for OwnedRegistryKey {
    fn drop(&mut self) {
        // SAFETY: 句柄由 open_current_user 成功取得，且只在此处关闭一次。
        let _ = unsafe { RegCloseKey(self.handle()) };
    }
}

/// 以整数保存可跨线程共享的内核事件句柄，所有权仍只属于该对象。
struct OwnedEvent(usize);

impl OwnedEvent {
    /// 创建单等待者使用的自动重置事件。
    fn create() -> io::Result<Self> {
        // SAFETY: 不传安全属性和名称，返回的有效句柄由 Self 接管。
        let handle = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }
            .map_err(io::Error::other)?;
        Ok(Self(handle.0 as usize))
    }

    /// 唤醒等待该事件的线程。
    fn signal(&self) -> io::Result<()> {
        // SAFETY: handle 在 Self 析构前始终有效。
        unsafe { SetEvent(self.handle()) }.map_err(io::Error::other)
    }

    /// 返回仅供 Win32 调用使用的非拥有句柄值。
    fn handle(&self) -> HANDLE {
        raw_handle(self.0)
    }

    /// 返回可复制到监听线程的非拥有句柄表示。
    fn raw(&self) -> usize {
        self.0
    }
}

impl Drop for OwnedEvent {
    fn drop(&mut self) {
        // SAFETY: 句柄由 create 成功取得，且只在此处关闭一次。
        let _ = unsafe { CloseHandle(self.handle()) };
    }
}

/// 将保存的 Win32 句柄值重建为非拥有 HANDLE。
fn raw_handle(value: usize) -> HANDLE {
    HANDLE(value as *mut c_void)
}

/// 保留 Win32 原始错误码，便于上层区分“键不存在”和真实监听故障。
fn win32_result(error: windows::Win32::Foundation::WIN32_ERROR) -> io::Result<()> {
    if error.is_ok() {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(error.0 as i32))
    }
}
