import type { ApplicationLocale } from '@/features/i18n/locales'
import type { LyricsOnlineStrategy } from '@/features/lyrics/types'
import type { MediaSessionSelectionStrategy } from '@/features/media/types'
import type {
  TaskbarOverlapPriority,
  TaskbarPlacement,
  TaskbarWidthMode,
} from '@/features/taskbar/contracts'
import { DEFAULT_BRAND_COLOR_HEX, DEFAULT_LYRICS_UNPLAYED_COLOR_HEX } from '@/features/theme/colors'
import type { UpdateCheckFrequency, UpdateCheckResult } from '@/features/updater/settings'

import type { TaskbarAudioSpectrumSettings } from './audio-spectrum'
import type { TaskbarBackgroundStyle } from './background-style'
import type { TaskbarAutoHide } from './bar-visibility'
import type { TaskbarCoverAppearance } from './cover'
import type { TaskbarLyricsNetworkPolicy, TaskbarLyricsSettings } from './lyrics'
import nativeDefaultsJson from './native-defaults.json'
import type { TaskbarProgressPosition, TaskbarProgressStyle } from './progress-style'
import type { TaskbarThemeColor } from './theme-color'
import type { TaskbarTrackInfoAlignment, TaskbarTrackInfoScrolling } from './track-info'

/**
 * 应用设置的前端默认值来源。
 *
 * 各设置模块只从这里读取默认值，再于自身模块内完成规范化、持久化与广播；
 * 调整默认值只需修改本文件（原生启动期同样需要的取值见下方共享数据）。
 * 类型一律使用 `import type` 引入，因此本模块运行期只依赖 `@/features/theme/colors`
 * 与 `./native-defaults.json`，不构成循环导入。
 *
 * 由域枚举派生的默认值仍留在各自模块内，因为它们必须跟随枚举本身变化：
 * `taskbar.elementOrder`、`taskbar.controls.visibility`（整份默认值，其 `order`
 * 与 `TASKBAR_CONTROL_BUTTONS` 一一对应）、`media.sessionSelection.playerPriority`。
 * 若搬到这里，本文件将反向依赖设置模块，并触发初始化期的暂时性死区错误。
 */

/**
 * `native-defaults.json` 承载原生启动期必须自行读取的取值——那时前端尚未运行，
 * 无法依赖前端推送。因此宽度范围与 `taskbar.width` / `widthMode` / `placement` /
 * `overlapPriority` / `displayTarget` / `lyrics` 的默认值以该文件为唯一来源，
 * 原生侧通过 `src-tauri/src/native_defaults.rs` 读取同一份数据。
 *
 * JSON 导入只能推导出 `string`，不会保留字面量联合类型，所以在此集中断言一次；
 * 枚举取值的合法性由原生侧的 serde 枚举在启动解析时校验。
 *
 * 同一份文件还承载存储结构版本号（原生侧需要自行判废过期的 `taskbar.lyrics`），
 * 由 `./storage/schema-versions.ts` 读取。
 *
 * 写入契约：原生侧只在启动时读取上述键，之后不再从存储同步。修改它们必须经由各自的
 * setter（`setTaskbarWidth`、`setTaskbarPlacement`、`setTaskbarOverlapPriority`、
 * `setTaskbarDisplayTarget`、`setTaskbarLyricsSettings`），因为"推送原生"与"写入存储"
 * 是在那里成对完成的。绕过 setter 直接写 `settingsStore` 会让原生保持旧值而设置界面
 * 显示新值，并且没有任何机制会自动纠正。
 *
 * `media.sessionSelection` 遵循同一条规则，只是它的推送经由变更事件而非同一个函数。
 */
const sharedDefaults = nativeDefaultsJson as {
  taskbar: {
    widthMin: number
    widthMax: number
    width: number
    widthMode: TaskbarWidthMode
    placement: TaskbarPlacement
    overlapPriority: TaskbarOverlapPriority
    displayTarget: string
    lyrics: {
      enabled: boolean
      networkPolicy: TaskbarLyricsNetworkPolicy
      onlineStrategy: LyricsOnlineStrategy
    }
  }
  media: {
    selectionStrategy: MediaSessionSelectionStrategy
    onlySupportedPlayers: boolean
  }
}

// ---- application ----

/** 界面语言，缺失或损坏时回退简体中文。 */
export const DEFAULT_APPLICATION_LOCALE: ApplicationLocale = 'zh-Hans'

/** 应用级减少动态效果覆盖项默认关闭，由系统偏好优先。 */
export const DEFAULT_REDUCED_MOTION = false

/** 默认开启应用级自动更新检测。 */
export const DEFAULT_AUTOMATIC_UPDATE_CHECK = true

/** 默认每日检测更新。 */
export const DEFAULT_UPDATE_CHECK_FREQUENCY: UpdateCheckFrequency = 'daily'

/** 尚未完成任何一次更新检测时的初始结果。 */
export const DEFAULT_UPDATE_CHECK_RESULT: UpdateCheckResult = {
  checkedAt: 0,
  availableVersion: null,
}

// ---- taskbar ----

/** 任务栏背景透明度默认完全不透明。 */
export const DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY = 0

/** 背景样式默认使用封面模糊。 */
export const DEFAULT_TASKBAR_BACKGROUND_STYLE: TaskbarBackgroundStyle = 'cover-blur'

/** bar 基准宽度的可调下限；原生侧用同一份取值做钳制。 */
export const TASKBAR_WIDTH_MIN = sharedDefaults.taskbar.widthMin

/** bar 基准宽度的可调上限；原生侧用同一份取值做钳制。 */
export const TASKBAR_WIDTH_MAX = sharedDefaults.taskbar.widthMax

/** bar 基准宽度默认值；与可调范围解耦，可单独调整。 */
export const DEFAULT_TASKBAR_WIDTH = sharedDefaults.taskbar.width

/** 宽度模式默认固定宽度。 */
export const DEFAULT_TASKBAR_WIDTH_MODE: TaskbarWidthMode = sharedDefaults.taskbar.widthMode

/** 播放器默认自动避让任务栏空间。 */
export const DEFAULT_TASKBAR_PLACEMENT: TaskbarPlacement = sharedDefaults.taskbar.placement

/** 遮挡优先级默认保证播放器完整显示。 */
export const DEFAULT_TASKBAR_OVERLAP_PRIORITY: TaskbarOverlapPriority =
  sharedDefaults.taskbar.overlapPriority

/** 全屏时默认在全部任务栏显示。 */
export const ALL_TASKBAR_DISPLAYS = sharedDefaults.taskbar.displayTarget

/** 播放进度默认显示为底部横条。 */
export const DEFAULT_TASKBAR_PROGRESS_STYLE: TaskbarProgressStyle = 'bottom'

/** 横条进度默认位于下边缘。 */
export const DEFAULT_TASKBAR_PROGRESS_POSITION: TaskbarProgressPosition = 'bottom'

/** 播放进度默认可见。 */
export const DEFAULT_TASKBAR_PROGRESS_VISIBLE = true

/** 默认无媒体会话时自动隐藏，暂停时不隐藏。 */
export const DEFAULT_TASKBAR_AUTO_HIDE: TaskbarAutoHide = {
  whenPaused: false,
  whenNoMediaSession: true,
}

/** 封面默认始终显示、圆角、播放时旋转，并标注播放器来源。 */
export const DEFAULT_TASKBAR_COVER_APPEARANCE: TaskbarCoverAppearance = {
  visibility: 'always',
  shape: 'rounded',
  rotateWhenPlaying: true,
  showPlayerSource: true,
}

/** 主题色默认取自封面。 */
export const DEFAULT_TASKBAR_THEME_COLOR: TaskbarThemeColor = {
  source: 'cover',
  customColor: DEFAULT_BRAND_COLOR_HEX,
}

/** 歌词显示默认双行、左对齐、逐字高亮并跟随主题配色。 */
export const DEFAULT_TASKBAR_LYRICS_SETTINGS: TaskbarLyricsSettings = {
  enabled: sharedDefaults.taskbar.lyrics.enabled,
  alignment: 'left',
  lineMode: 'double',
  secondaryLine: 'translation_or_next',
  networkPolicy: sharedDefaults.taskbar.lyrics.networkPolicy,
  onlineStrategy: sharedDefaults.taskbar.lyrics.onlineStrategy,
  timingOffsetMs: 0,
  wordHighlight: true,
  animation: 'up',
  animationPreRoll: true,
  fontSize: 14,
  colorScheme: 'theme',
  playedColor: DEFAULT_BRAND_COLOR_HEX,
  unplayedColor: DEFAULT_LYRICS_UNPLAYED_COLOR_HEX,
  fontFamily: '',
}

/** 歌曲信息默认显示并左对齐。 */
export const DEFAULT_TASKBAR_TRACK_INFO_VISIBLE = true

/** 歌曲信息默认左对齐。 */
export const DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT: TaskbarTrackInfoAlignment = 'left'

/** 歌名溢出默认循环滚动。 */
export const DEFAULT_TASKBAR_TRACK_INFO_SCROLLING: TaskbarTrackInfoScrolling = {
  enabled: true,
  speed: 25,
  mode: 'loop',
}

/** 频谱默认隐藏、底部对齐、24 条、占据 45% 宽度。 */
export const DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS: TaskbarAudioSpectrumSettings = {
  visible: false,
  barCount: 24,
  widthPercentage: 45,
  alignment: 'bottom',
  horizontalPosition: 100,
  sensitivity: 100,
  smoothing: 55,
  frameRate: 20,
}

// ---- media ----

/** 多播放器并存时默认按最近播放选择会话。 */
export const DEFAULT_MEDIA_SESSION_SELECTION_STRATEGY: MediaSessionSelectionStrategy =
  sharedDefaults.media.selectionStrategy

/** 默认只把受支持的播放器纳入候选。 */
export const DEFAULT_MEDIA_SESSION_ONLY_SUPPORTED_PLAYERS =
  sharedDefaults.media.onlySupportedPlayers
