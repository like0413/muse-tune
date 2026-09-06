//! 通过 Windows UI Automation 读取任务栏实际交互元素的边界。

use windows::{
    Win32::{
        Foundation::{E_FAIL, HWND},
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize, SAFEARRAY,
        },
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
}

impl TaskbarLayoutEventHandler_Impl {
    /// 仅接受任务栏 Provider 自身的事件，排除作为拥有窗口出现的 bar 控件。
    fn notify_if_taskbar_provider(
        &self,
        sender: Ref<'_, IUIAutomationElement>,
    ) -> windows::core::Result<()> {
        let sender = sender.ok()?;
        if unsafe { sender.CachedProcessId()? } == self.provider_process_id {
            request_layout_update();
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

/// 复用监控线程的 COM 单元与 UI Automation 客户端。
pub(super) struct TaskbarElements {
    automation: Option<IUIAutomation>,
    query: Option<TaskbarQuery>,
    subscription: Option<TaskbarEventSubscription>,
}

impl TaskbarElements {
    /// 在当前监控线程初始化 UI Automation；不可用时由调用方保持完整 bar。
    pub(super) fn new() -> Option<Self> {
        // SAFETY: 任务栏监控使用独立线程，并在本类型 Drop 时于同一线程配对反初始化。
        let initialize_result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if initialize_result.is_err() {
            log::warn!("初始化任务栏 UI Automation 的 COM 单元失败: {initialize_result:?}");
            return None;
        }

        // SAFETY: CUIAutomation 是系统注册的进程内 COM 类，返回接口由 windows crate 管理。
        let automation = unsafe {
            CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
        };
        match automation {
            Ok(automation) => Some(Self {
                automation: Some(automation),
                query: None,
                subscription: None,
            }),
            Err(error) => {
                log::warn!("创建任务栏 UI Automation 客户端失败: {error}");
                // SAFETY: 上方 CoInitializeEx 已成功，且当前线程尚未持有 COM 接口。
                unsafe { CoUninitialize() };
                None
            }
        }
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
        drop(self.automation.take());
        // SAFETY: new 仅在 CoInitializeEx 成功后构造 Self，且 Drop 与构造发生在同一线程。
        unsafe { CoUninitialize() };
    }
}

/// 为任务栏根元素注册布局失效与结构变化事件。
fn subscribe_taskbar_events(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    provider_process_id: i32,
) -> windows::core::Result<TaskbarEventSubscription> {
    let layout_handler: IUIAutomationEventHandler = TaskbarLayoutEventHandler {
        provider_process_id,
    }
    .into();
    let structure_handler = layout_handler.cast::<IUIAutomationStructureChangedEventHandler>()?;
    let event_cache = create_property_cache(automation, UIA_ProcessIdPropertyId)?;

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
    let root = unsafe { automation.ElementFromHandle(taskbar)? };
    let provider_process_id = unsafe { root.CurrentProcessId()? };
    let button_condition = unsafe {
        automation.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &VARIANT::from(UIA_ButtonControlTypeId.0),
        )?
    };
    let process_condition = unsafe {
        automation
            .CreatePropertyCondition(UIA_ProcessIdPropertyId, &VARIANT::from(provider_process_id))?
    };
    let visible_condition = unsafe {
        automation.CreatePropertyCondition(UIA_IsOffscreenPropertyId, &VARIANT::from(false))?
    };
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
    let cache = unsafe { automation.CreateCacheRequest()? };
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
    let elements = unsafe {
        query
            .root
            .FindAllBuildCache(TreeScope_Descendants, &query.condition, &query.cache)?
    };
    let length = unsafe { elements.Length()? };
    let mut rects = Vec::with_capacity(length.max(0) as usize);

    for index in 0..length {
        let element = unsafe { elements.GetElement(index)? };
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
