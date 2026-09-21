//! 通过 Windows UI Automation 读取任务栏实际交互元素的边界。

use std::{marker::PhantomData, rc::Rc};

use windows::{
    Win32::{
        Foundation::{E_FAIL, HWND},
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize, SAFEARRAY,
        },
        System::Threading::GetCurrentThreadId,
        System::Variant::VARIANT,
        UI::Accessibility::{
            AutomationElementMode_None, CUIAutomation, IUIAutomation, IUIAutomationCacheRequest,
            IUIAutomationCondition, IUIAutomationElement, IUIAutomationEventHandler,
            IUIAutomationEventHandler_Impl, IUIAutomationStructureChangedEventHandler,
            IUIAutomationStructureChangedEventHandler_Impl, StructureChangeType,
            TreeScope_Descendants, TreeScope_Subtree, UIA_BoundingRectanglePropertyId,
            UIA_ButtonControlTypeId, UIA_ControlTypePropertyId, UIA_EVENT_ID,
            UIA_IsOffscreenPropertyId, UIA_LayoutInvalidatedEventId, UIA_ProcessIdPropertyId,
        },
    },
    core::{Interface, Ref, implement},
};

use super::{events::request_layout_update, geometry::ScreenRect};

/// 将任务栏 UIA Provider 的布局与结构事件转发给已有监控消息循环。
#[implement(IUIAutomationEventHandler, IUIAutomationStructureChangedEventHandler)]
struct TaskbarLayoutEventHandler {
    provider_process_id: i32,
    monitor_thread_id: u32,
}

impl TaskbarLayoutEventHandler_Impl {
    /// 仅接受任务栏 Provider 自身的事件，排除作为拥有窗口出现的 bar 控件。
    fn notify_if_taskbar_provider(
        &self,
        sender: Ref<'_, IUIAutomationElement>,
    ) -> windows::core::Result<()> {
        let sender = sender.ok()?;
        // SAFETY: 事件回调只在订阅它的同步线程上触发，sender 由 UIA 在该次回调期间保证有效；
        // 读取的是注册处理器时请求过的缓存属性，不会再回到 Explorer 里现场取一次值。
        if unsafe { sender.CachedProcessId()? } == self.provider_process_id {
            request_layout_update(self.monitor_thread_id);
        }
        Ok(())
    }
}

#[allow(non_snake_case)]
impl IUIAutomationEventHandler_Impl for TaskbarLayoutEventHandler_Impl {
    fn HandleAutomationEvent(
        &self,
        sender: Ref<'_, IUIAutomationElement>,
        _event_id: UIA_EVENT_ID,
    ) -> windows::core::Result<()> {
        self.notify_if_taskbar_provider(sender)
    }
}

#[allow(non_snake_case)]
impl IUIAutomationStructureChangedEventHandler_Impl for TaskbarLayoutEventHandler_Impl {
    fn HandleStructureChangedEvent(
        &self,
        sender: Ref<'_, IUIAutomationElement>,
        _change_type: StructureChangeType,
        _runtime_id: *const SAFEARRAY,
    ) -> windows::core::Result<()> {
        self.notify_if_taskbar_provider(sender)
    }
}

/// 持有任务栏 UIA 事件订阅所需的根元素与处理器。
struct TaskbarEventSubscription {
    root: IUIAutomationElement,
    layout_handler: IUIAutomationEventHandler,
    structure_handler: IUIAutomationStructureChangedEventHandler,
}

/// 缓存任务栏根元素、Provider 端过滤条件和跨进程属性请求。
struct TaskbarQuery {
    root: IUIAutomationElement,
    provider_process_id: i32,
    condition: IUIAutomationCondition,
    cache: IUIAutomationCacheRequest,
}

/// 当前 taskbar sync worker 独占的 COM MTA。
struct ComMta {
    _thread_affinity: PhantomData<Rc<()>>,
}

impl ComMta {
    /// 初始化当前线程 COM apartment。
    fn initialize() -> windows::core::Result<Self> {
        // SAFETY: taskbar sync worker 是独立线程，本类型不离开创建线程。
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        Ok(Self {
            _thread_affinity: PhantomData,
        })
    }
}

impl Drop for ComMta {
    fn drop(&mut self) {
        // SAFETY: initialize 成功后才构造 Self，且所有 COM 接口已由 TaskbarElements 先释放。
        unsafe { CoUninitialize() };
    }
}

/// 复用监控线程的 COM 单元与 UI Automation 客户端。
///
/// `_apartment` 必须留在最后一个字段：Rust 按声明顺序析构，它会先释放全部 COM 接口、
/// 最后才 `CoUninitialize`。把它挪到前面会让 COM 单元先于接口失效，之后释放接口即未定义行为。
pub(super) struct TaskbarElements {
    automation: Option<IUIAutomation>,
    query: Option<TaskbarQuery>,
    subscription: Option<TaskbarEventSubscription>,
    _apartment: ComMta,
}

impl TaskbarElements {
    /// 在当前监控线程初始化 UI Automation；不可用时由调用方保持完整 bar。
    pub(super) fn new() -> Option<Self> {
        let apartment = ComMta::initialize()
            .inspect_err(|error| log::warn!("初始化任务栏 UI Automation 的 COM 单元失败: {error}"))
            .ok()?;

        // SAFETY: CUIAutomation 是系统注册的进程内 COM 类，返回接口由 windows crate 管理。
        let automation = match unsafe {
            CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
        } {
            Ok(automation) => automation,
            Err(error) => {
                log::warn!("创建任务栏 UI Automation 客户端失败: {error}");
                return None;
            }
        };
        Some(Self {
            automation: Some(automation),
            query: None,
            subscription: None,
            _apartment: apartment,
        })
    }

    /// 订阅任务栏 Provider 的布局事件；Explorer 重建时替换旧订阅。
    pub(super) fn watch_taskbar(&mut self, taskbar: HWND) {
        self.remove_subscription();
        self.query = None;
        let Some(automation) = self.automation.as_ref() else {
            return;
        };

        let query = match create_taskbar_query(automation, taskbar) {
            Ok(query) => query,
            Err(error) => {
                log::warn!("初始化任务栏 UI Automation 查询失败: {error}");
                return;
            }
        };

        match subscribe_taskbar_events(automation, &query.root, query.provider_process_id) {
            Ok(subscription) => self.subscription = Some(subscription),
            Err(error) => {
                log::warn!("订阅任务栏 UI Automation 布局事件失败，将使用低频查询兜底: {error}");
            }
        }
        self.query = Some(query);
    }

    /// 判断 UIA 布局事件是否已成功订阅。
    pub(super) fn is_event_driven(&self) -> bool {
        self.subscription.is_some()
    }

    /// 从 UI Automation 客户端移除当前任务栏事件订阅。
    fn remove_subscription(&mut self) {
        let Some(subscription) = self.subscription.take() else {
            return;
        };
        let Some(automation) = self.automation.as_ref() else {
            return;
        };

        // SAFETY: automation 与 subscription 里的接口都创建于本线程的 MTA，且只能在创建它们的
        // 线程上调用；subscription 在移除前才被 take，处理器的所有权一直由它持有，因此这里传回
        // 的正是注册时的那两个实例。
        unsafe {
            let _ = automation.RemoveAutomationEventHandler(
                UIA_LayoutInvalidatedEventId,
                &subscription.root,
                &subscription.layout_handler,
            );
            let _ = automation.RemoveStructureChangedEventHandler(
                &subscription.root,
                &subscription.structure_handler,
            );
        }
    }

    /// 批量读取当前可见任务栏按钮的屏幕矩形。
    pub(super) fn button_rects(
        &self,
        taskbar_rect: ScreenRect,
        tray_rect: Option<ScreenRect>,
    ) -> windows::core::Result<Vec<ScreenRect>> {
        let Some(query) = self.query.as_ref() else {
            return Err(windows::core::Error::new(
                E_FAIL,
                "任务栏 UI Automation 查询尚未初始化",
            ));
        };

        read_button_rects(query, taskbar_rect, tray_rect)
    }
}

impl Drop for TaskbarElements {
    fn drop(&mut self) {
        self.remove_subscription();
        // 必须先释放 COM 接口，再反初始化创建它们的线程单元。
        drop(self.query.take());
        drop(self.automation.take());
    }
}

/// 为任务栏根元素注册布局失效与结构变化事件。
fn subscribe_taskbar_events(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    provider_process_id: i32,
) -> windows::core::Result<TaskbarEventSubscription> {
    // SAFETY: 订阅始终由所属任务栏的同步线程创建。
    let monitor_thread_id = unsafe { GetCurrentThreadId() };
    let layout_handler: IUIAutomationEventHandler = TaskbarLayoutEventHandler {
        provider_process_id,
        monitor_thread_id,
    }
    .into();
    let structure_handler = layout_handler.cast::<IUIAutomationStructureChangedEventHandler>()?;
    let event_cache = create_property_cache(automation, UIA_ProcessIdPropertyId)?;

    // SAFETY: 注册与调用都发生在所属任务栏的同步线程（同一 MTA）上；两个 handler 的所有权
    // 交给返回的 TaskbarEventSubscription，注销时用的就是同一对实例。订阅跨进程生效，若 Explorer
    // 正在重建，注册可能失败并返回错误，此时调用方退回低频查询。
    unsafe {
        automation.AddAutomationEventHandler(
            UIA_LayoutInvalidatedEventId,
            root,
            TreeScope_Subtree,
            &event_cache,
            &layout_handler,
        )?;
        if let Err(error) = automation.AddStructureChangedEventHandler(
            root,
            TreeScope_Subtree,
            &event_cache,
            &structure_handler,
        ) {
            let _ = automation.RemoveAutomationEventHandler(
                UIA_LayoutInvalidatedEventId,
                root,
                &layout_handler,
            );
            return Err(error);
        }
    }

    Ok(TaskbarEventSubscription {
        root: root.clone(),
        layout_handler,
        structure_handler,
    })
}

/// 为当前 Explorer 任务栏创建可跨采样复用的 UIA 查询。
fn create_taskbar_query(
    automation: &IUIAutomation,
    taskbar: HWND,
) -> windows::core::Result<TaskbarQuery> {
    // SAFETY: taskbar 由调用方在同步线程上验证过；查询跨进程建立，Explorer 正在退出时会失败，
    // 错误直接上抛，调用方据此跳过本次订阅并保留完整 bar。
    let root = unsafe { automation.ElementFromHandle(taskbar)? };
    // SAFETY: root 来自本次查询，只读取该元素的进程 ID；跨进程读取可能失败，同样由 `?` 上抛。
    let provider_process_id = unsafe { root.CurrentProcessId()? };
    // SAFETY: 条件由本线程的 UIA 客户端创建，VARIANT 是临时值且在调用期间存活。
    let button_condition = unsafe {
        automation.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &VARIANT::from(UIA_ButtonControlTypeId.0),
        )?
    };
    // SAFETY: 同上，条件与临时 VARIANT 都只在本次调用期间使用。
    let process_condition = unsafe {
        automation
            .CreatePropertyCondition(UIA_ProcessIdPropertyId, &VARIANT::from(provider_process_id))?
    };
    // SAFETY: 同上，条件与临时 VARIANT 都只在本次调用期间使用。
    let visible_condition = unsafe {
        automation.CreatePropertyCondition(UIA_IsOffscreenPropertyId, &VARIANT::from(false))?
    };
    // SAFETY: 三个输入条件在调用期间由上面的局部变量持有，因此组合条件不会引用到已释放的对象。
    let condition = unsafe {
        automation.CreateAndConditionFromNativeArray(&[
            Some(button_condition),
            Some(process_condition),
            Some(visible_condition),
        ])?
    };
    let cache = create_property_cache(automation, UIA_BoundingRectanglePropertyId)?;

    Ok(TaskbarQuery {
        root,
        provider_process_id,
        condition,
        cache,
    })
}

/// 创建只保留指定缓存属性、不建立完整元素引用的跨进程查询请求。
fn create_property_cache(
    automation: &IUIAutomation,
    property: windows::Win32::UI::Accessibility::UIA_PROPERTY_ID,
) -> windows::core::Result<IUIAutomationCacheRequest> {
    // SAFETY: 缓存请求由本线程的 UIA 客户端创建，随后由本函数设置的属性与模式都只作用于它。
    let cache = unsafe { automation.CreateCacheRequest()? };
    // SAFETY: 属性 ID 由调用方传入且来自 UIA 的固定枚举；模式只控制是否建立完整元素引用。
    unsafe {
        cache.AddProperty(property)?;
        cache.SetAutomationElementMode(AutomationElementMode_None)?;
    }
    Ok(cache)
}

/// 使用已缓存条件一次读取任务栏 Provider 的可见按钮边界。
fn read_button_rects(
    query: &TaskbarQuery,
    taskbar_rect: ScreenRect,
    tray_rect: Option<ScreenRect>,
) -> windows::core::Result<Vec<ScreenRect>> {
    // SAFETY: 跨进程批量查询，调用方已保证 query 在同线程创建并持有；Explorer 正在重建时可能
    // 直接失败，或返回一批已经失效的元素，两种情况都由下面逐元素读取缓存属性时体现。
    let elements = unsafe {
        query
            .root
            .FindAllBuildCache(TreeScope_Descendants, &query.condition, &query.cache)?
    };
    // SAFETY: 只读取本次结果集的长度，不移动或释放其中的元素。
    let length = unsafe { elements.Length()? };
    let mut rects = Vec::with_capacity(length.max(0) as usize);

    for index in 0..length {
        // SAFETY: index 一定落在上面取到的长度范围内。
        let element = unsafe { elements.GetElement(index)? };
        // SAFETY: 只读取注册缓存请求时声明过的属性；元素若已失效这里返回错误，跳过即可。
        let rect = ScreenRect::from(unsafe { element.CachedBoundingRectangle()? });
        let horizontal_center = rect.left + rect.width() / 2;
        let belongs_to_tray = tray_rect
            .is_some_and(|tray| horizontal_center >= tray.left && horizontal_center < tray.right);
        if rect.width() > 0
            && rect.height() > 0
            && rect.intersects(taskbar_rect)
            && !belongs_to_tray
        {
            rects.push(rect);
        }
    }

    Ok(rects)
}
