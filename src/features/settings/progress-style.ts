import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import {
  DEFAULT_TASKBAR_PROGRESS_POSITION,
  DEFAULT_TASKBAR_PROGRESS_STYLE,
  DEFAULT_TASKBAR_PROGRESS_VISIBLE,
} from './defaults'
import { settingsStore } from './store'

const TASKBAR_PROGRESS_STYLES = ['bottom', 'cover-ring', 'vertical-gradient'] as const
const TASKBAR_PROGRESS_POSITIONS = ['top', 'bottom'] as const
const TASKBAR_PROGRESS_STYLE_KEY = 'taskbar.progressStyle'
const TASKBAR_PROGRESS_POSITION_KEY = 'taskbar.progressPosition'
const TASKBAR_PROGRESS_VISIBLE_KEY = 'taskbar.progressVisible'
const TASKBAR_PROGRESS_STYLE_CHANGED_EVENT = 'settings://taskbar-progress-style-changed'
const TASKBAR_PROGRESS_POSITION_CHANGED_EVENT = 'settings://taskbar-progress-position-changed'
const TASKBAR_PROGRESS_VISIBLE_CHANGED_EVENT = 'settings://taskbar-progress-visible-changed'

export type TaskbarProgressStyle = (typeof TASKBAR_PROGRESS_STYLES)[number]
export type TaskbarProgressPosition = (typeof TASKBAR_PROGRESS_POSITIONS)[number]

export {
  DEFAULT_TASKBAR_PROGRESS_POSITION,
  DEFAULT_TASKBAR_PROGRESS_STYLE,
  DEFAULT_TASKBAR_PROGRESS_VISIBLE,
}

/** 读取进度条显隐，旧配置默认显示。 */
export async function getTaskbarProgressVisible(): Promise<boolean> {
  const visible = await settingsStore.get<unknown>(TASKBAR_PROGRESS_VISIBLE_KEY)
  return typeof visible === 'boolean' ? visible : DEFAULT_TASKBAR_PROGRESS_VISIBLE
}

export async function setTaskbarProgressVisible(visible: boolean): Promise<void> {
  await settingsStore.set(TASKBAR_PROGRESS_VISIBLE_KEY, visible)
  await emit(TASKBAR_PROGRESS_VISIBLE_CHANGED_EVENT, visible)
}

export async function listenTaskbarProgressVisibleChange(
  handler: (visible: boolean) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_PROGRESS_VISIBLE_CHANGED_EVENT, ({ payload }) => {
    if (typeof payload === 'boolean') handler(payload)
  })
}

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

export async function setTaskbarProgressStyle(style: TaskbarProgressStyle): Promise<void> {
  await settingsStore.set(TASKBAR_PROGRESS_STYLE_KEY, style)
  await emit(TASKBAR_PROGRESS_STYLE_CHANGED_EVENT, style)
}

/** 读取横条进度的上下位置。 */
export async function getTaskbarProgressPosition(): Promise<TaskbarProgressPosition> {
  const position = await settingsStore.get<unknown>(TASKBAR_PROGRESS_POSITION_KEY)
  return isTaskbarProgressPosition(position) ? position : DEFAULT_TASKBAR_PROGRESS_POSITION
}

export async function setTaskbarProgressPosition(position: TaskbarProgressPosition): Promise<void> {
  await settingsStore.set(TASKBAR_PROGRESS_POSITION_KEY, position)
  await emit(TASKBAR_PROGRESS_POSITION_CHANGED_EVENT, position)
}

export async function listenTaskbarProgressStyleChange(
  handler: (style: TaskbarProgressStyle) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_PROGRESS_STYLE_CHANGED_EVENT, ({ payload }) => {
    if (isTaskbarProgressStyle(payload)) {
      handler(payload)
    }
  })
}

export async function listenTaskbarProgressPositionChange(
  handler: (position: TaskbarProgressPosition) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_PROGRESS_POSITION_CHANGED_EVENT, ({ payload }) => {
    if (isTaskbarProgressPosition(payload)) handler(payload)
  })
}
