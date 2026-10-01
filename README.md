<div align="center">

<img src="./src-tauri/icons/app-icon.png" alt="MuseTune 图标" width="112" />

# MuseTune

把正在播放的音乐带到 Windows 11 任务栏。

![Windows 11](https://img.shields.io/badge/Windows-11-0078D4?style=flat-square&logo=windows11&logoColor=white)
![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white)
![Vue 3](https://img.shields.io/badge/Vue-3-42B883?style=flat-square&logo=vuedotjs&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-2024-000000?style=flat-square&logo=rust&logoColor=white)

[官网](https://musetune.like0413.cn) · [下载](https://github.com/like0413/muse-tune/releases) · [功能亮点](#功能亮点) · [安装与使用](#安装与使用) · [本地开发](#本地开发)

</div>

MuseTune 是一款面向 Windows 11 的任务栏媒体控制器。它通过 Windows 系统媒体会话读取当前歌曲，在任务栏中展示封面、歌曲信息、歌词、播放进度和实时频谱，并提供常用播放控制。

> [!NOTE]
> MuseTune 依赖播放器向 Windows 发布媒体会话。歌曲信息、进度和控制能力以播放器实际提供的内容为准。

## 功能亮点

- **融入任务栏**：原生跟踪任务栏位置、显示器与布局变化，支持自动或左右停靠、水平偏移、固定或自动宽度及多显示器；可选择遮挡优先级，以及无媒体会话或暂停时自动隐藏。
- **常用媒体控制**：播放/暂停、上一首、下一首、静音和滚轮调节音量，可选择当前播放器音量或 Windows 系统主音量。
- **同步歌词**：支持单行/双行、逐字高亮、翻译或下一行、简繁转换、字体、配色、时间偏移和多种动画；可使用播放器本地缓存、MuseTune 缓存与在线来源，支持仅本地模式，以及仅当前平台或多来源并行查询。
- **丰富的视觉选项**：可分别调整普通模式和歌词模式的封面与元素顺序，支持旋转封面、模糊封面背景、封面取色或自定义主题色、歌曲信息滚动、顶部/底部进度条、全高渐变、封面进度环和实时音频频谱。
- **多播放器选择**：可按最近播放、Windows 当前会话、固定优先级或保持当前播放器选择媒体会话。
- **完整桌面体验**：系统托盘快捷开关、开机自启、自动检查更新、缓存与日志管理、运行诊断，以及简体中文、繁體中文和 English 界面；支持减少动态效果和 GPU 加速设置。

## 安装与使用

### 系统要求

- Windows 11 x64
- 一个能够发布 Windows 系统媒体会话的音乐播放器

从 [官网](https://musetune.like0413.cn) 或 [GitHub Releases](https://github.com/like0413/muse-tune/releases) 下载。发布流程提供两种构建：

- `MuseTune-*-windows-x64.exe`：安装版，支持应用内下载并安装更新。
- `MuseTune-*-windows-x64-portable.zip`：便携版，解压后直接运行 `MuseTune.exe`，无需安装。

便携版首次运行会在 `MuseTune.exe` 相邻位置创建 `data/`，其中分别保存 `config/`、`cache/`、`logs/` 和 `webview/`。移动便携版时请连同 `data/` 一起移动；更新时退出应用、覆盖 `MuseTune.exe`，不要删除 `data/`。便携版可以检查新版本，但不会自动运行安装器，更新按钮会打开 Releases 下载页。

1. 启动 MuseTune，并在受支持的播放器中开始播放音乐。
2. MuseTune 默认按最近播放选择受支持的媒体会话，并在所有显示器的任务栏中显示组件。
3. 右键单击系统托盘中的 MuseTune 图标可打开设置、切换常用显示项、重启或退出应用。
4. 在设置中调整任务栏位置、布局、歌词、主题、频谱和播放器选择策略。

> [!TIP]
> 如果任务栏中没有出现内容，请先确认播放器已开始播放，并在“诊断”页检查媒体会话、任务栏窗口和音频能力。

### 日常操作与默认行为

- 播放中且同步歌词可用时，默认显示双行歌词；鼠标移入组件后显示歌曲信息和播放控件，移出后恢复歌词。暂停或没有同步歌词时显示普通模式。
- 在组件上滚动鼠标滚轮可按 2% 调节音量，默认控制 Windows 系统主音量；可在设置中改为当前播放器音量。音量面板在鼠标停留期间保持显示，移出后收起。
- 右键单击任务栏组件可显示或隐藏当前播放器窗口；应用设置、重启和退出入口位于系统托盘菜单。
- 默认使用固定宽度和自动位置：任务栏图标居中时放在左侧，否则放在右侧。空间不足时默认优先避让任务栏项目。
- 默认在没有媒体会话时隐藏，暂停时保留；频谱默认关闭，可从托盘菜单或设置开启。
- 新版本会通过自动检查、系统通知和托盘更新入口提示，也可在设置的“关于”页手动检查。安装版确认后下载并安装，便携版前往 Releases 手动更新。

“常规”页可调整语言、明暗外观、开机自启和减少动态效果。GPU 加速默认禁用，修改后需要重启应用；该设置可能影响 CPU 负载和动画流畅度，可按设备情况调整。

## 支持的播放器

MuseTune 为下列 Windows 桌面播放器提供专用识别与适配：

| 播放器     | 媒体控制 | 播放器本地歌词缓存 | 在线歌词 |
| ---------- | :------: | :----------------: | :------: |
| QQ 音乐    |    ✓     |         ✓          |    ✓     |
| 网易云音乐 |    ✓     |         ✓          |    ✓     |
| 汽水音乐   |    ✓     |         —          |    ✓     |
| 酷狗音乐   |    ✓     |         ✓          |    ✓     |
| Spotify    |    ✓     |         ✓          |    —     |

关闭“仅支持的播放器”后，其他发布 Windows 媒体会话的应用也可参与会话选择，但不会获得播放器专用的歌词与兼容性适配。

> [!IMPORTANT]
> 歌词可用性取决于播放器版本、缓存状态、歌曲元数据和网络来源；表格表示代码中已接入的解析能力，并不保证每首歌曲都能取得歌词。

并行策略可在 QQ 音乐、酷狗音乐、汽水音乐和网易云音乐之间选择在线来源与优先顺序；默认启用 QQ 音乐和酷狗音乐。当前播放平台具备在线能力时，其接口始终参与查询。选择“仅当前平台”后，只查询当前播放器自己的接口。Spotify 没有专用在线歌词接口，但在并行策略下可使用已启用的其他平台作为在线备用来源。

## 常见问题

**为什么看不到任务栏组件？**

默认没有媒体会话时会隐藏组件。先让播放器开始播放，再在“诊断”页确认是否识别到媒体会话，检查“任务栏”设置中的显示器与自动隐藏选项。其他应用需要关闭“仅支持的播放器”才能参与会话选择。

**为什么没有歌词或逐字高亮？**

本地歌词依赖播放器缓存，在线歌词依赖网络和歌曲匹配；逐字高亮还需要歌词本身提供逐字时间信息。可在“诊断”页查看歌词来源和解析状态，并在“数据”页重新获取当前歌词或清除缓存。

**为什么频谱没有显示？**

频谱默认关闭。开启后仍需当前播放器存在可采集的 Windows 音频会话；可在“诊断”页检查音频会话和频谱采集状态。

遇到其他问题，可通过 [GitHub Issues](https://github.com/like0413/muse-tune/issues) 或 [在线反馈](https://my.feishu.cn/share/base/form/shrcnn3ULhbTAYPtfst2h9fdSqf) 提交，在“诊断”页复制诊断报告，并附上播放器版本和复现步骤。

## 本地开发

### 前置环境

- Windows 11
- [Git](https://git-scm.com/)
- Node.js `^22.18.0` 或 `>=24.12.0`
- pnpm `11.24.0`
- [Vite+](https://viteplus.dev/guide/) 全局 CLI（`vp`）
- Rust `>=1.98.0`（发布流程使用 `1.98.0`），包含 `rustfmt`、`clippy` 和 `x86_64-pc-windows-msvc` target
- Visual Studio 2022 C++ 生成工具与 Windows SDK

### 启动应用

```powershell
git clone https://github.com/like0413/muse-tune.git
cd muse-tune
vp install
vp exec tauri dev
```

`vp exec tauri dev` 会自动启动前端开发服务和 Tauri 桌面应用。便携模式开发可使用 `vp exec tauri dev --features portable`。

如果只需要调试 Vue 前端，可运行 `vp dev`，地址为 `http://localhost:21480`；媒体会话、音量、歌词和任务栏集成等原生能力需要在 Tauri 中验证。

### 代码检查

```powershell
vp check
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --locked --features portable -- -D warnings
```

## 技术架构

- **Vue 3 + TypeScript**：任务栏界面、设置、状态管理与国际化。
- **Tauri 2 + Rust**：应用生命周期、托盘、更新、IPC 与原生服务。
- **Windows API**：GSMTC 媒体会话、Core Audio、显示器与任务栏窗口集成。
- **Vite+ + Tailwind CSS**：开发工具链、检查流程与界面样式。

```text
src/
├─ features/        # 媒体、歌词、任务栏、设置、更新等前端能力
├─ pages/taskbar/   # 任务栏播放器界面
├─ pages/settings/  # 设置、数据管理、诊断与更新界面
└─ locales/         # 简体中文、繁體中文与 English 文案

src-tauri/
├─ src/media/       # Windows 媒体会话、音量与频谱采集
├─ src/lyrics/      # 歌词发现、解析、缓存与播放器适配
├─ src/taskbar/     # 任务栏窗口布局、同步与多显示器管理
├─ src/storage.rs   # 安装版与便携版数据路径
└─ capabilities/    # Tauri 窗口权限边界
```

## 当前边界

- 仅面向 Windows 11；其他操作系统和旧版 Windows 不在支持范围内。
- 部分播放器只在播放期间创建媒体或音频会话，暂停、退出或切换设备时可用能力会随之变化。
- 实时频谱需要当前播放器存在可采集的 Windows 音频会话。
