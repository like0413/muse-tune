import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import {
  DEFAULT_TASKBAR_PROGRESS_STYLE,
  getTaskbarProgressStyle,
  setTaskbarProgressStyle,
  type TaskbarProgressStyle,
} from './progress-style'
import { settingsStore } from './store'

const BACKGROUND_STYLE_KEY = 'taskbar.backgroundStyle'
const BACKGROUND_STYLE_CHANGED_EVENT = 'settings://taskbar-background-style-changed'
const BACKGROUND_FLOW_KEY = 'taskbar.backgroundFlow'
const BACKGROUND_FLOW_CHANGED_EVENT = 'settings://taskbar-background-flow-changed'
export const DEFAULT_TASKBAR_BACKGROUND_STYLE = 'theme'
export const DEFAULT_TASKBAR_BACKGROUND_FLOW = false
export type TaskbarBackgroundStyle = 'theme' | 'cover-blur'

/** 验证持久化的背景样式。 */
export function isTaskbarBackgroundStyle(value: unknown): value is TaskbarBackgroundStyle {
  return value === 'theme' || value === 'cover-blur'
}

/** 读取背景样式；旧版 cover-flow 迁移为模糊封面加独立流动开关。 */
export async function getTaskbarBackgroundStyle(): Promise<TaskbarBackgroundStyle> {
  const value = await settingsStore.get<unknown>(BACKGROUND_STYLE_KEY)
  if (value === 'cover-flow') return 'cover-blur'
  return isTaskbarBackgroundStyle(value) ? value : DEFAULT_TASKBAR_BACKGROUND_STYLE
}

/** 读取流动偏好；尚未迁移时兼容旧版 cover-flow 配置。 */
export async function getTaskbarBackgroundFlow(): Promise<boolean> {
  const value = await settingsStore.get<unknown>(BACKGROUND_FLOW_KEY)
  if (typeof value === 'boolean') return value
  return (await settingsStore.get<unknown>(BACKGROUND_STYLE_KEY)) === 'cover-flow'
}

/** 首次保存背景样式前固化旧版流动偏好，避免切回主题背景时丢失。 */
async function migrateTaskbarBackgroundFlow(): Promise<void> {
  const value = await settingsStore.get<unknown>(BACKGROUND_FLOW_KEY)
  if (typeof value === 'boolean') return
  await settingsStore.set(
    BACKGROUND_FLOW_KEY,
    (await settingsStore.get<unknown>(BACKGROUND_STYLE_KEY)) === 'cover-flow',
  )
}

/** 保存背景样式并同步任务栏窗口。 */
async function saveTaskbarBackgroundStyle(style: TaskbarBackgroundStyle): Promise<void> {
  await migrateTaskbarBackgroundFlow()
  await settingsStore.set(BACKGROUND_STYLE_KEY, style)
  await emit(BACKGROUND_STYLE_CHANGED_EVENT, style)
}

/** 保存流动偏好；关闭模糊封面时仍保留该选择。 */
export async function setTaskbarBackgroundFlow(enabled: boolean): Promise<void> {
  await settingsStore.set(BACKGROUND_FLOW_KEY, enabled)
  await emit(BACKGROUND_FLOW_CHANGED_EVENT, enabled)
}

/** 切换封面背景时，将冲突的全高进度效果恢复为横条。 */
export async function setTaskbarBackgroundStyle(style: TaskbarBackgroundStyle): Promise<boolean> {
  const previousProgress = await getTaskbarProgressStyle()
  const replacedProgress = style !== 'theme' && previousProgress === 'vertical-gradient'
  if (replacedProgress) await setTaskbarProgressStyle(DEFAULT_TASKBAR_PROGRESS_STYLE)
  try {
    await saveTaskbarBackgroundStyle(style)
  } catch (error) {
    if (replacedProgress) await setTaskbarProgressStyle(previousProgress)
    throw error
  }
  return replacedProgress
}

/** 切换全高进度效果时，将冲突的封面背景恢复为主题背景。 */
export async function setCompatibleTaskbarProgressStyle(
  style: TaskbarProgressStyle,
): Promise<boolean> {
  const previousBackground = await getTaskbarBackgroundStyle()
  const replacedBackground = style === 'vertical-gradient' && previousBackground !== 'theme'
  if (replacedBackground) await saveTaskbarBackgroundStyle(DEFAULT_TASKBAR_BACKGROUND_STYLE)
  try {
    await setTaskbarProgressStyle(style)
  } catch (error) {
    if (replacedBackground) await saveTaskbarBackgroundStyle(previousBackground)
    throw error
  }
  return replacedBackground
}

/** 接收设置窗口的背景样式变化。 */
export async function listenTaskbarBackgroundStyleChange(
  handler: (style: TaskbarBackgroundStyle) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(BACKGROUND_STYLE_CHANGED_EVENT, ({ payload }) => {
    if (isTaskbarBackgroundStyle(payload)) handler(payload)
  })
}

/** 接收设置窗口的流动偏好变化。 */
export async function listenTaskbarBackgroundFlowChange(
  handler: (enabled: boolean) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(BACKGROUND_FLOW_CHANGED_EVENT, ({ payload }) => {
    if (typeof payload === 'boolean') handler(payload)
  })
}
