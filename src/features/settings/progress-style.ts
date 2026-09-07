import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

const TASKBAR_PROGRESS_STYLES = ['bottom', 'vertical-gradient'] as const
const TASKBAR_PROGRESS_POSITIONS = ['top', 'bottom'] as const
const TASKBAR_PROGRESS_STYLE_KEY = 'taskbar.progressStyle'
const TASKBAR_PROGRESS_POSITION_KEY = 'taskbar.progressPosition'
const TASKBAR_PROGRESS_STYLE_CHANGED_EVENT = 'settings://taskbar-progress-style-changed'
const TASKBAR_PROGRESS_POSITION_CHANGED_EVENT = 'settings://taskbar-progress-position-changed'

export type TaskbarProgressStyle = (typeof TASKBAR_PROGRESS_STYLES)[number]
export type TaskbarProgressPosition = (typeof TASKBAR_PROGRESS_POSITIONS)[number]

export const DEFAULT_TASKBAR_PROGRESS_STYLE: TaskbarProgressStyle = 'bottom'
export const DEFAULT_TASKBAR_PROGRESS_POSITION: TaskbarProgressPosition = 'bottom'

/** 判断外部值是否为受支持的播放进度样式。 */
export function isTaskbarProgressStyle(value: unknown): value is TaskbarProgressStyle {
  return typeof value === 'string' && TASKBAR_PROGRESS_STYLES.some((style) => style === value)
}

/** 判断外部值是否为受支持的横条位置。 */
export function isTaskbarProgressPosition(value: unknown): value is TaskbarProgressPosition {
  return (
    typeof value === 'string' && TASKBAR_PROGRESS_POSITIONS.some((position) => position === value)
  )
}

/** 读取播放进度样式，缺失或损坏时使用底部横条。 */
export async function getTaskbarProgressStyle(): Promise<TaskbarProgressStyle> {
  const style = await settingsStore.get<unknown>(TASKBAR_PROGRESS_STYLE_KEY)
  return isTaskbarProgressStyle(style) ? style : DEFAULT_TASKBAR_PROGRESS_STYLE
}

/** 持久化播放进度样式，并通知全部任务栏窗口立即切换。 */
export async function setTaskbarProgressStyle(style: TaskbarProgressStyle): Promise<void> {
  await settingsStore.set(TASKBAR_PROGRESS_STYLE_KEY, style)
  await emit(TASKBAR_PROGRESS_STYLE_CHANGED_EVENT, style)
}

/** 读取横条进度的上下位置。 */
export async function getTaskbarProgressPosition(): Promise<TaskbarProgressPosition> {
  const position = await settingsStore.get<unknown>(TASKBAR_PROGRESS_POSITION_KEY)
  return isTaskbarProgressPosition(position) ? position : DEFAULT_TASKBAR_PROGRESS_POSITION
}

/** 持久化横条位置，并通知全部任务栏窗口。 */
export async function setTaskbarProgressPosition(position: TaskbarProgressPosition): Promise<void> {
  await settingsStore.set(TASKBAR_PROGRESS_POSITION_KEY, position)
  await emit(TASKBAR_PROGRESS_POSITION_CHANGED_EVENT, position)
}

/** 监听设置窗口发出的播放进度样式变更。 */
export async function listenTaskbarProgressStyleChange(
  handler: (style: TaskbarProgressStyle) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_PROGRESS_STYLE_CHANGED_EVENT, ({ payload }) => {
    if (isTaskbarProgressStyle(payload)) {
      handler(payload)
    }
  })
}

/** 监听横条进度位置变更。 */
export async function listenTaskbarProgressPositionChange(
  handler: (position: TaskbarProgressPosition) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_PROGRESS_POSITION_CHANGED_EVENT, ({ payload }) => {
    if (isTaskbarProgressPosition(payload)) handler(payload)
  })
}
