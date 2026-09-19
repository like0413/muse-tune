/** Windows 任务栏对应的显示器摘要。 */
export interface TaskbarDisplay {
  id: string
  label: string
  width: number
  height: number
  isPrimary: boolean
}

/** 原生任务栏支持的播放器布局位置。 */
export const TASKBAR_PLACEMENTS = ['auto', 'left', 'right'] as const
/** 原生任务栏支持的遮挡优先级。 */
export const TASKBAR_OVERLAP_PRIORITIES = ['bar', 'taskbar'] as const
/** 原生任务栏支持的宽度模式：固定宽度或自适应任务栏空白。 */
export const TASKBAR_WIDTH_MODES = ['fixed', 'auto'] as const

export type TaskbarPlacement = (typeof TASKBAR_PLACEMENTS)[number]
export type TaskbarOverlapPriority = (typeof TASKBAR_OVERLAP_PRIORITIES)[number]
export type TaskbarWidthMode = (typeof TASKBAR_WIDTH_MODES)[number]

/** 托盘菜单项要求任务栏窗口执行的动作。 */
export const TRAY_MENU_ACTIONS = ['normal-cover', 'lyrics-cover', 'lyrics', 'spectrum'] as const

export type TrayMenuAction = (typeof TRAY_MENU_ACTIONS)[number]

/** 托盘菜单文案，由任务栏窗口按当前界面语言提供。 */
export interface TrayMenuLabels {
  normalCover: string
  lyricsCover: string
  lyrics: string
  spectrum: string
  settings: string
  restart: string
  quit: string
}

/** 托盘菜单开关项的勾选状态。 */
export interface TrayMenuChecked {
  normalCover: boolean
  lyricsCover: boolean
  lyrics: boolean
  spectrum: boolean
}

/** 托盘菜单完整展示状态；原生菜单只负责渲染。 */
export interface TrayMenuPresentation {
  labels: TrayMenuLabels
  checked: TrayMenuChecked
}

/** 判断事件负载是否为受支持的托盘菜单动作。 */
export function isTrayMenuAction(value: unknown): value is TrayMenuAction {
  return TRAY_MENU_ACTIONS.some((action) => action === value)
}
