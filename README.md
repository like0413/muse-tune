<div align="center">

<img src="./src-tauri/icons/app-icon.png" alt="MuseTune 图标" width="112" />

# MuseTune

把正在播放的音乐带到 Windows 11 任务栏。

![Windows 11](https://img.shields.io/badge/Windows-11-0078D4?style=flat-square&logo=windows11&logoColor=white)
![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white)
![Vue 3](https://img.shields.io/badge/Vue-3-42B883?style=flat-square&logo=vuedotjs&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-2024-000000?style=flat-square&logo=rust&logoColor=white)

[功能亮点](#功能亮点) · [安装与使用](#安装与使用) · [支持的播放器](#支持的播放器) · [本地开发](#本地开发)

</div>

MuseTune 是一款面向 Windows 11 的任务栏媒体控制器。它通过 Windows 系统媒体会话读取当前歌曲，在任务栏中展示封面、歌曲信息、歌词、播放进度和实时频谱，并提供常用播放控制。

> [!NOTE]
> MuseTune 依赖播放器向 Windows 发布媒体会话。歌曲信息、进度和控制能力以播放器实际提供的内容为准。

## 功能亮点

- **融入任务栏**：原生跟踪任务栏位置、显示器与布局变化，支持左右停靠、水平偏移、固定或自动宽度及多显示器。
- **常用媒体控制**：播放/暂停、上一首、下一首，以及当前播放器音量或 Windows 系统主音量控制。
- **同步歌词**：支持单行/双行、逐字高亮、翻译或下一行、简繁转换、时间偏移和多种动画；可组合播放器本地缓存与在线来源。
- **丰富的视觉选项**：封面、模糊封面背景、主题色、歌曲信息滚动、底部进度条、全高渐变、封面进度环和实时音频频谱。
- **多播放器选择**：可按最近播放、Windows 当前会话、固定优先级或保持当前播放器选择媒体会话。
- **完整桌面体验**：系统托盘快捷开关、开机自启、应用内更新、缓存与日志管理、运行诊断，以及简体中文、繁體中文和 English 界面。

## 安装与使用

### 系统要求

- Windows 11 x64
- 一个能够发布 Windows 系统媒体会话的音乐播放器

项目发布版本时会在 [GitHub Releases](https://github.com/like0413/muse-tune/releases) 同时提供两种构建：

- `MuseTune-*-windows-x64.exe`：安装版，支持应用内下载并安装更新。
- `MuseTune-*-windows-x64-portable.zip`：便携版，解压后直接运行 `MuseTune.exe`，无需安装。

便携版首次运行会在 `MuseTune.exe` 相邻位置创建 `data/`，其中分别保存 `config/`、`cache/`、`logs/` 和 `webview/`。移动便携版时请连同 `data/` 一起移动；更新时退出应用、覆盖 `MuseTune.exe`，不要删除 `data/`。便携版可以检查新版本，但不会自动运行安装器，更新按钮会打开 Releases 下载页。

1. 在受支持的播放器中开始播放音乐。
2. MuseTune 会自动选择媒体会话，并在任务栏中显示播放器。
3. 右键单击系统托盘中的 MuseTune 图标可打开设置、切换常用显示项、重启或退出应用。
4. 在设置中调整任务栏位置、布局、歌词、主题、频谱和播放器选择策略。

> [!TIP]
> 如果任务栏中没有出现内容，请先确认播放器已开始播放，并在“诊断”页检查媒体会话、任务栏窗口和音频能力。

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

## 本地开发

### 前置环境

- Windows 11
- [Git](https://git-scm.com/)
- Node.js `^22.18.0` 或 `>=24.12.0`
- pnpm `11.24.0`
- [Vite+](https://viteplus.dev/guide/) 全局 CLI（`vp`）
- Rust `1.98.0`，包含 `rustfmt`、`clippy` 和 `x86_64-pc-windows-msvc` target
- Visual Studio 2022 C++ 生成工具与 Windows SDK

### 启动应用

```powershell
git clone https://github.com/like0413/muse-tune.git
cd muse-tune
vp install
vp exec tauri dev
```

`vp exec tauri dev` 会启动 Tauri 桌面应用；如果只需要调试 Vue 前端，可运行 `vp dev`。

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
├─ features/        # 按媒体、歌词、任务栏、设置等领域组织的前端能力
├─ pages/taskbar/   # 任务栏播放器界面
├─ pages/settings/  # 设置、数据管理、诊断与更新界面
└─ locales/         # 简体中文、繁體中文与 English 文案

src-tauri/
├─ src/media/       # Windows 媒体会话、音量与频谱采集
├─ src/lyrics/      # 歌词发现、解析、缓存与播放器适配
├─ src/taskbar/     # 任务栏窗口布局、同步与多显示器管理
└─ capabilities/    # Tauri 窗口权限边界
```

## 当前边界

- 仅面向 Windows 11；其他操作系统和旧版 Windows 不在支持范围内。
- 部分播放器只在播放期间创建媒体或音频会话，暂停、退出或切换设备时可用能力会随之变化。
- 实时频谱需要当前播放器存在可采集的 Windows 音频会话。
