# Windows 11 任务栏嵌入方案

> 状态：技术预研 / 待原型验证  
> 适用项目：Muse Tune  
> 当前技术栈：Windows 11、Tauri 2、Rust、Vue、WebView2

## 1. 文档范围

本文只讨论 Muse Tune 如何显示在 Windows 11 任务栏区域，不讨论 QQ 音乐、网易云音乐、汽水音乐、酷狗音乐的数据获取与控制适配。

本文保留以下四种候选方案：

1. Owner 任务栏伴随窗口；
2. Tauri `WebviewWindow` 直接变为任务栏子窗口；
3. 原生 Win32 Host + Tauri Child WebView；
4. 原生 Win32 Host + WRY/WebView2 Child WebView。

以下路线不纳入核心候选：

- Explorer/XAML 注入：需要进入 `explorer.exe` 并依赖 Windows 私有实现，兼容性和安全风险过高；
- AppBar：位于任务栏旁边并占用桌面工作区，不是嵌入任务栏；
- Windows Widgets：内容显示在 Widgets Board，而不是常驻任务栏横条；
- DeskBand/任务栏 Toolbar：不适合作为 Windows 11 新项目方案；
- 托盘图标、任务栏按钮和缩略图工具栏：无法承载常驻封面、歌曲信息、进度条和歌词。

## 2. 基本事实与术语

Windows 11 没有向普通桌面应用开放“在任务栏 XAML 布局中注册任意自定义控件”的稳定公共 API。因此，本文四种方案都不能让 Windows 自动为 Muse Tune 分配布局槽位。

无论采用哪种方案，应用都需要自行完成：

- 查找主任务栏 `Shell_TrayWnd`；
- 查找副屏任务栏 `Shell_SecondaryTrayWnd`；
- 验证任务栏窗口确实属于 `explorer.exe`；
- 测量开始按钮、Widgets、应用按钮组、通知区域和时钟的边界；
- 选择未被占用的任务栏矩形；
- 处理任务栏居中/左对齐、自动隐藏和多显示器；
- 在 Explorer 重启后重新创建或重新挂载窗口；
- 在全屏应用、Win+D、锁屏、睡眠和 RDP 等状态下维护可见性与 Z-order。

### 2.1 Parent 与 Owner 的区别

**Parent/Child：**

- 子窗口使用父窗口客户区坐标；
- 子窗口受到父窗口可见性、边界和 Z-order 的影响；
- `SetParent` 不会自动修改 `WS_CHILD` 和 `WS_POPUP`，调用方必须同步窗口样式；
- Muse Tune 与 Explorer 会形成跨进程 HWND 父子树，需要重点验证 DPI Awareness。

**Owner/Owned：**

- Muse Tune 仍是独立顶层窗口，继续使用屏幕坐标；
- Owned 窗口保持在 Owner 上方，并且不会作为普通应用窗口出现在任务栏；
- Owner 关系不会自动让窗口跟随任务栏移动，仍需主动同步位置；
- 不会形成跨进程父子 HWND 树，DPI 模型比 `SetParent` 更容易控制。

### 2.2 “看起来嵌入”和“结构嵌入”

| 方案            | 用户视觉 | Win32 结构             | 进入 Windows 11 任务栏 XAML 布局 |
| --------------- | -------- | ---------------------- | -------------------------------- |
| Owner 伴随窗口  | 嵌入     | 独立顶层窗口           | 否                               |
| 三种 Child 方案 | 嵌入     | `Shell_TrayWnd` 子窗口 | 否                               |

即使窗口成为 `Shell_TrayWnd` 的子窗口，它也只是位于任务栏 HWND 范围内，不会参与开始按钮、任务按钮和系统托盘的 XAML 自动布局。

## 3. 共享基础模块

四种方案应共用同一套任务栏探测与布局代码，避免将平台逻辑散落在 Vue 页面或各个窗口回调中。

建议的 Rust 模块边界：

```text
TaskbarDockManager
├── TaskbarLocator
├── TaskbarLayoutProbe
├── TaskbarPlacement
├── DpiMapper
├── LifecycleWatcher
├── FullscreenGuard
└── AttachmentStrategy
    ├── OwnedOverlayAttachment
    ├── TauriWindowChildAttachment
    ├── TauriChildWebViewAttachment
    └── WryChildWebViewAttachment
```

### 3.1 TaskbarLocator

职责：

- 使用 `EnumWindows` 枚举任务栏，而不是无条件信任第一次 `FindWindow` 的结果；
- 检查窗口类名；
- 使用 `GetWindowThreadProcessId` 验证所属进程为 `explorer.exe`；
- 将任务栏与显示器绑定；
- 为显示器保存稳定标识，不能只持久化 `DISPLAY1`、`DISPLAY2` 之类可能变化的编号。

### 3.2 TaskbarLayoutProbe

Windows 11 的任务栏主体是 XAML，经典子 HWND 无法可靠反映全部任务按钮的真实宽度。建议：

- 使用 Win32 `GetWindowRect` 获取任务栏整体边界；
- 主屏通知区域可以使用 `TrayNotifyWnd` 作为快速参考；
- 使用 UI Automation 查找 `StartButton`、`WidgetsButton`、任务栏应用按钮和通知区域；
- UI Automation 在后台线程执行并缓存结果，不能放在逐帧渲染或高频主线程回调中；
- 探测失败时保留最后一次有效结果，不能立即把窗口移动到可能覆盖系统按钮的位置。

### 3.3 TaskbarPlacement

布局层统一使用物理像素，最终再根据目标任务栏的 DPI 转换为 WebView 的逻辑尺寸。

建议支持响应式宽度：

```text
空间充足：封面 + 歌曲 + 歌手 + 控制按钮 + 进度条 + 歌词
空间一般：封面 + 歌曲/歌手 + 核心控制按钮
空间较小：封面 + 播放/暂停
没有安全空间：隐藏任务栏窗口，保留托盘入口
```

任何模式都不得覆盖开始按钮、任务按钮、Widgets、输入法、通知区域、时钟和显示桌面区域。

### 3.4 LifecycleWatcher

至少处理：

- `TaskbarCreated`：Explorer 重建任务栏后重新枚举和挂载；
- `WM_DISPLAYCHANGE`：显示器连接、断开或主屏切换；
- `WM_SETTINGCHANGE`：任务栏设置、主题、对齐或自动隐藏变化；
- `WM_DPICHANGED`：目标显示器缩放变化；
- `EVENT_OBJECT_LOCATIONCHANGE`：通过 `SetWinEventHook` 低成本监听任务栏移动和尺寸变化；
- 低频校准定时器：用于恢复丢失的 Z-order 或处理未产生可靠事件的 Shell 状态变化。

## 4. 方案一：Owner 任务栏伴随窗口

### 4.1 窗口层级

```text
Desktop
├── Shell_TrayWnd              Owner
└── Muse Tune WebviewWindow    Owned top-level window
    └── WebView2
```

### 4.2 实现方式

1. 创建正常的 Tauri `WebviewWindow`；
2. 设置无边框、透明、跳过任务栏、不可通过普通方式调整大小；
3. 获取 Tauri 窗口 HWND；
4. 保留 `WS_POPUP` 顶层窗口属性；
5. 添加 `WS_EX_TOOLWINDOW`；
6. 根据交互需求添加 `WS_EX_NOACTIVATE`；
7. 调用 `SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, taskbar_hwnd)` 设置 Owner；
8. 调用 `SetWindowPos` 使窗口覆盖在计算出的任务栏空白区域；
9. 在任务栏位置、DPI、可见性和 Z-order 变化时同步窗口。

### 4.3 优点

- 完整保留 Tauri `WebviewWindow`、Vue、IPC、事件、能力权限和 WebView2 生命周期；
- 不需要启用 Tauri `unstable` feature；
- 不建立跨进程父子 HWND 树，规避 `SetParent` 的主要 DPI 风险；
- 窗口继续使用屏幕坐标，多屏和混合 DPI 更容易处理；
- Muse Tune 崩溃不会影响 Explorer；
- 最适合当前项目快速实现和长期发布。

### 4.4 缺点与风险

- 技术上不是任务栏子窗口；
- Owner 关系不负责几何位置，必须持续同步；
- 自动隐藏、全屏应用、Win+D 和虚拟桌面需要额外处理；
- 某些 Shell 状态下窗口可能仍是 `WS_VISIBLE`，但 DWM 暂停合成，需要低频重新提交 Z-order；
- 使用 `WS_EX_NOACTIVATE` 后不适合依赖键盘输入，任务栏播放器应以鼠标交互为主；
- Explorer 重启后 Owner HWND 失效，必须重新关联或重建窗口。

### 4.5 Tauri 注意事项

- 这是四种方案中最接近 Tauri 官方稳定使用方式的方案；
- 不应只依靠任务栏 WebView 维持应用生命周期；
- 建议保留隐藏的生命周期/消息窗口或托盘入口；
- Rust 侧统一执行原生窗口操作，Vue 侧不应自行计算屏幕坐标；
- 窗口创建应放在 Tauri `setup` 阶段或安全的异步路径，避免在同步 IPC 回调中创建 WebView2 窗口。

### 4.6 定位

**推荐作为默认正式方案。**

它不能在技术定义上实现真正的 HWND 子窗口嵌入，但能够实现用户感知上的任务栏原生体验，并且最符合当前 Tauri 架构。

## 5. 方案二：Tauri WebviewWindow 直接变为任务栏子窗口

### 5.1 窗口层级

```text
Shell_TrayWnd
└── Tauri WebviewWindow HWND
    └── WebView2
```

### 5.2 实现方式

1. 隐藏创建 Tauri `WebviewWindow`；
2. 获取它的顶层 HWND；
3. 保存原始 Parent、`GWL_STYLE`、`GWL_EXSTYLE` 和窗口矩形，便于恢复；
4. 清除 `WS_POPUP`、`WS_CAPTION`、`WS_THICKFRAME`、最小化/最大化等样式；
5. 添加 `WS_CHILD | WS_CLIPSIBLINGS`；
6. 调用 `SetParent(tauri_hwnd, taskbar_hwnd)`；
7. 使用任务栏客户区相对坐标调用 `SetWindowPos`；
8. 必要时使用 `SetWindowRgn` 限制绘制与命中区域；
9. Explorer 重启前后重新创建窗口或完整解除并重新挂载。

### 5.3 优点

- HWND 结构上确实成为任务栏子窗口；
- 不需要自己创建 Win32 Host；
- 继续使用完整的 Tauri IPC、Vue 页面与 WebView2 配置；
- 子窗口自然受到父窗口可见性和边界的影响；
- 是验证 `SetParent` 路线最快的原型方案。

### 5.4 缺点与风险

- Tauri/TAO 仍可能把该 HWND 当作正常顶层窗口管理；
- 嵌入后调用 Tauri 的居中、外部坐标、最大化、阴影或窗口特效可能产生错误结果；
- 跨进程 `SetParent` 存在 DPI Awareness 不匹配和进程 DPI 模式被重置的风险；
- 子坐标和屏幕坐标混用很容易产生双倍缩放或偏移；
- Explorer 销毁父窗口时，子窗口也会被销毁；
- Windows 更新或第三方任务栏工具可能改变层级、裁剪和合成表现。

### 5.5 Tauri 注意事项

- 嵌入成功后，位置、尺寸和 Z-order 应完全交给 Win32 适配层管理；
- Tauri 窗口 API 只保留显示、隐藏、事件和 IPC 等不会改变原生层级的操作；
- 在 `SetParent` 前使用 `GetWindowDpiAwarenessContext` 检查双方 DPI Context；
- 挂载失败必须恢复原始窗口样式和 Parent；
- 不应将任务栏窗口设为应用退出的唯一判断依据。

### 5.6 定位

**推荐作为第一个 Child 原型。**

如果它能通过完整的多屏、DPI、Explorer 重启和 WebView2 透明测试，就没有必要立即引入更加复杂的原生 Host。

## 6. 方案三：原生 Win32 Host + Tauri Child WebView

### 6.1 窗口层级

```text
Shell_TrayWnd
└── MuseTuneHost HWND
    └── Tauri-managed Child WebView
        └── WebView2
```

### 6.2 实现方式

1. Rust 侧注册专用 Win32 窗口类；
2. 创建 `MuseTuneHost`，负责窗口消息、DPI、挂载、裁剪和生命周期；
3. 将 Host 设置为任务栏子窗口；
4. 使用 Tauri `WindowBuilder` 创建可承载子 WebView 的窗口宿主；
5. 使用 `WebviewBuilder` 和 `Window::add_child` 创建 Tauri 管理的 Child WebView；
6. Host 尺寸变化时同步 Child WebView bounds；
7. Explorer 重启时销毁旧 Host 和 Child WebView，再完整重建。

### 6.3 优点

- 原生窗口职责和 Web UI 职责分离最清晰；
- Host 可以完整处理 `WndProc`、DPI、区域裁剪和 Explorer 生命周期；
- Tauri Child WebView 仍能使用 Tauri 的 IPC、安全协议、事件和资源加载；
- 长期比“直接改变 Tauri 顶层窗口语义”更容易维护。

### 6.4 缺点与风险

- 当前 Tauri 的 `WebviewBuilder` 和 `Window::add_child` 仍需要 `unstable` feature；
- 项目会绑定 Tauri 内部能力，升级 Tauri 时需要重点回归；
- Host 与 Tauri Window 的关系需要谨慎设计，不能出现两层互相竞争的尺寸和生命周期管理；
- Windows 上 WebView2 创建存在消息线程和重入限制；
- 实现和测试成本明显高于方案二。

### 6.5 Tauri 注意事项

- 需要把 `tauri` 依赖显式启用 `unstable` feature；
- 子 WebView 必须拥有唯一 label，并加入对应 Tauri capability 的 `windows` 或 `webviews` 范围；
- 所有 Child WebView 的创建、销毁和 bounds 更新必须在正确的线程执行；
- 不要在同步 Tauri command 或 WebView 事件处理器里直接创建 Child WebView；
- 必须确认 Tauri 更新后 `Window::add_child` 的行为和 feature 标记是否变化。

### 6.6 定位

**作为长期候选，不作为第一版默认方案。**

只有方案二证明 Tauri 顶层窗口重设 Parent 会持续与 TAO 冲突，并且 Child WebView API 在目标 Tauri 版本上足够稳定时，才值得迁移到该方案。

## 7. 方案四：原生 Win32 Host + WRY/WebView2 Child WebView

### 7.1 窗口层级

```text
Shell_TrayWnd
└── MuseTuneHost HWND
    └── WRY_WEBVIEW HWND 或 WebView2 Controller
        └── Vue UI
```

### 7.2 实现方式

1. 创建原生 Win32 Host 并挂入任务栏；
2. 使用 WRY `WebViewBuilder::build_as_child`，或者直接使用 WebView2 COM API；
3. 配置 WebView2 Environment、数据目录、透明背景和页面加载；
4. 自行实现 Rust 与前端之间的消息协议；
5. 自行提供前端资源协议、CSP、导航限制、开发服务器切换和错误处理；
6. 自行管理 WebView2 Controller 与 Host 的创建、缩放、销毁和重建。

### 7.3 优点

- 对 Win32 Host 和 WebView2 拥有最高控制力；
- 不依赖 Tauri 的 Child WebView `unstable` API；
- WRY 官方支持在 Windows 父窗口中构建 Child WebView；
- 可以针对任务栏场景精简窗口行为和资源加载流程。

### 7.4 缺点与风险

- 绕过 Tauri WebView 管理层后，现成的 Tauri IPC、事件、能力权限和自定义协议不能直接使用；
- 实际上需要自行维护一套缩小版 Tauri Runtime；
- Vue 开发服务器、生产资源、CSP、导航白名单和数据目录都需要单独处理；
- WRY 和 Tauri 可能各自初始化 WebView2 Environment，必须防止配置冲突；
- 后续安全审计和升级成本最高。

### 7.5 Tauri 注意事项

- Tauri 可以继续负责主进程、托盘、设置窗口和更新，但任务栏 WebView 将脱离 Tauri 管理；
- 如果选择 WRY，应使用与当前 Tauri 锁定版本兼容的 WRY，避免同一进程出现多个不兼容版本；
- Rust 与 Vue 的通信协议必须进行来源校验、消息类型校验和错误隔离；
- 不能简单复制 Tauri 内部初始化脚本作为长期实现，因为内部协议不属于稳定公共接口。

### 7.6 定位

**仅作为兜底方案。**

只有前三种方案无法满足需求时才考虑。若采用它，应把任务栏 WebView 看作独立前端宿主，而不是普通的 Tauri 窗口。

## 8. 四种方案对比

评分采用 1～5，分数越高表示越有利；“实现风险”分数越高表示风险越高。

| 维度                  | Owner 伴随窗口 | Tauri 窗口直接 Child | Win32 Host + Tauri Child WebView | Win32 Host + WRY/WebView2 |
| --------------------- | -------------: | -------------------: | -------------------------------: | ------------------------: |
| 用户视觉一致性        |              4 |                    4 |                                4 |                         4 |
| HWND 结构嵌入         |              1 |                    5 |                                5 |                         5 |
| Tauri 集成完整度      |              5 |                    5 |                                4 |                         1 |
| API 稳定性            |              5 |                    3 |                                2 |                         4 |
| DPI 可控性            |              5 |                    2 |                                3 |                         3 |
| Explorer 重启恢复难度 |              3 |                    2 |                                3 |                         3 |
| 开发复杂度            |              5 |                    4 |                                2 |                         1 |
| 长期维护成本          |              5 |                    3 |                                3 |                         1 |
| 实现风险              |              2 |                    3 |                                4 |                         5 |

综合顺序：

1. Owner 伴随窗口：正式版本默认方案；
2. Tauri `WebviewWindow` 直接 Child：真正嵌入的第一原型；
3. Win32 Host + Tauri Child WebView：长期架构候选；
4. Win32 Host + WRY/WebView2：前三种失败后的兜底。

## 9. 推荐决策

建议不要在尚未完成原型测试时一次性决定最终实现，而是按以下顺序推进：

### 阶段一：共享探测层

实现 `TaskbarLocator`、`TaskbarLayoutProbe`、`DpiMapper` 和 `LifecycleWatcher`。这些工作四种方案都需要，不会因最终选择发生浪费。

### 阶段二：双原型对比

使用同一个简单测试条同时验证：

- Owner 伴随窗口；
- Tauri `WebviewWindow` 直接 Child。

原型阶段不接入播放器，不开发完整任务栏 UI，只验证窗口、透明、点击、DPI、生命周期和任务栏避让。

### 阶段三：选择默认挂载策略

- 如果 Owner 模式能满足自动隐藏、全屏、Win+D 和多屏体验，正式版采用 Owner；
- 如果产品必须具备结构上的任务栏子窗口关系，并且 Child 模式通过所有兼容测试，可以采用方案二；
- 如果方案二持续与 Tauri/TAO 窗口管理冲突，再验证方案三；
- 方案四不提前投入。

### 阶段四：保留安全回退

无论最终采用哪种方案，都应提供 `DetachedWindowAttachment`：当任务栏结构无法识别、第三方任务栏工具改变布局或挂载失败时，自动退回普通悬浮窗口或隐藏并保留托盘入口，不能强行修改未知任务栏结构。

## 10. 原型验证矩阵

### 10.1 Windows 与任务栏

- Windows 11 当前稳定版；
- 至少一个 Windows Insider 或下一功能更新环境；
- 任务栏居中与左对齐；
- 任务栏自动隐藏开/关；
- Widgets、搜索、任务视图开/关；
- 任务栏图标较少、较多和发生溢出；
- Explorer 手动重启和异常重启。

### 10.2 显示器与 DPI

- 单屏 100%、125%、150%、200%；
- 双屏相同 DPI；
- 双屏混合 DPI；
- 副屏位于主屏左侧、右侧、上方；
- 切换主显示器；
- 显示器热插拔；
- 副屏任务栏启用与禁用。

### 10.3 Shell 状态

- Win+D 显示桌面及恢复；
- 普通最大化窗口；
- 无边框全屏；
- 独占全屏游戏；
- 锁屏与解锁；
- 睡眠与恢复；
- RDP 连接、断开和分辨率变化；
- 虚拟桌面切换。

### 10.4 WebView2

- 透明背景无黑边、白闪和残影；
- 封面切换和 CSS 动画流畅；
- 鼠标点击、右键、滚轮和拖动进度条正常；
- 点击控件不抢走当前前台程序的键盘焦点；
- 页面刷新和 WebView2 进程异常退出后能够恢复；
- Explorer 重启后页面与 IPC 能够重新建立。

### 10.5 验收标准

- Explorer 重启后 3 秒内恢复显示；
- 不覆盖任何系统任务栏控件；
- 不改变其他应用的桌面工作区；
- 不因 DPI 转换错误出现双倍位移或尺寸；
- 不抢夺用户当前应用的键盘焦点；
- 无安全空间时自动缩小或隐藏；
- 挂载失败不会导致 Explorer 崩溃；
- 所有挂载操作都有明确错误日志和可恢复路径。

## 11. 实现约束

- 优先使用微软维护的 Rust `windows` crate 调用 Win32、COM 和 UI Automation API，避免手写大量 FFI 声明；
- 所有 Windows 专用依赖放在 `target.'cfg(windows)'.dependencies`；
- Windows 平台代码使用 `#[cfg(target_os = "windows")]` 隔离；
- 原生回调不得直接执行耗时 UI Automation、磁盘或网络操作；
- `SetWinEventHook` 回调需要防止重入，并把实际工作投递到受控线程；
- 所有 HWND 在使用前检查有效性和所属进程；
- 所有窗口样式修改都保存旧值，并提供完整恢复逻辑；
- 所有物理像素与逻辑单位转换集中在 `DpiMapper`；
- 不修改或 subclass 属于 Explorer 进程的窗口过程；
- 不在 Explorer 进程中注入代码。

## 12. 参考资料

### 官方资料

- [Microsoft Learn: SetParent function](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setparent)
- [Microsoft Learn: SetWindowLongPtrW function](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowlongptrw)
- [Microsoft Learn: Window Features](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features)
- [Microsoft Learn: The Taskbar / TaskbarCreated](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar)
- [Microsoft Learn: SetWinEventHook](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook)
- [Microsoft Learn: Application Desktop Toolbars](https://learn.microsoft.com/en-us/windows/win32/shell/application-desktop-toolbars)
- [Tauri 2: WebviewBuilder](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewBuilder.html)
- [WRY: Child webviews](https://docs.rs/wry/latest/wry/)

### 可参考项目

- [Now Playing - Taskbar Widget](https://github.com/mechanicwb2-hub/now-playing-taskbar-widget)：独立无边框任务栏覆盖窗口和动态位置计算；
- [Barline](https://github.com/madeitdisfar/Barline/blob/main/docs/design.md)：Owner 任务栏伴随窗口、Win+D、Z-order、多屏和 DPI 的设计记录；
- [Deskband11Lib](https://github.com/airtaxi/Deskband11Lib)：`SetParent`、UI Automation、任务栏空白区域和多屏恢复；
- [taskbar-lyric](https://github.com/apoint123/taskbar-lyric)：Rust 任务栏歌词窗口和 Windows 10/11 布局策略；
- [Tauri multi-webview example](https://github.com/tauri-apps/tauri/blob/dev/examples/multiwebview/main.rs)：Tauri Child WebView 官方示例；
- [WRY webview2 implementation](https://github.com/tauri-apps/wry/blob/dev/src/webview2/mod.rs)：Windows Child WebView 的底层实现。
