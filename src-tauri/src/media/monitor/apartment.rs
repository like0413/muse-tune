use std::{marker::PhantomData, rc::Rc};

use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};

/// 当前媒体 worker 独占的 WinRT MTA；必须晚于线程内 WinRT 对象释放。
pub(super) struct WinRtMta {
    _thread_affinity: PhantomData<Rc<()>>,
}

impl WinRtMta {
    /// 初始化当前线程的 WinRT MTA。
    pub(super) fn initialize() -> windows::core::Result<Self> {
        // SAFETY: 本类型只在专用 media worker 构造，并由同一线程析构。
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }?;
        Ok(Self {
            _thread_affinity: PhantomData,
        })
    }
}

impl Drop for WinRtMta {
    fn drop(&mut self) {
        // SAFETY: initialize 成功后才构造 Self，且 Self 不跨线程移动。
        unsafe { RoUninitialize() };
    }
}
