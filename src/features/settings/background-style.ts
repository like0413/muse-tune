import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { DEFAULT_TASKBAR_BACKGROUND_STYLE, DEFAULT_TASKBAR_PROGRESS_STYLE } from './defaults'
import {
  getTaskbarProgressStyle,
  setTaskbarProgressStyle,
  type TaskbarProgressStyle,
} from './progress-style'
import { settingsStore } from './store'

const BACKGROUND_STYLE_KEY = 'taskbar.backgroundStyle'
const BACKGROUND_STYLE_CHANGED_EVENT = 'settings://taskbar-background-style-changed'
export type TaskbarBackgroundStyle = 'theme' | 'cover-blur'
const FULL_HEIGHT_PROGRESS_BACKGROUND_STYLE: TaskbarBackgroundStyle = 'theme'

export { DEFAULT_TASKBAR_BACKGROUND_STYLE }

/** 验证持久化的背景样式。 */
export function isTaskbarBackgroundStyle(value: unknown): value is TaskbarBackgroundStyle {
  return value === 'theme' || value === 'cover-blur'
}

/** 读取背景样式，非法值回退到默认值。 */
export async function getTaskbarBackgroundStyle(): Promise<TaskbarBackgroundStyle> {
  const value = await settingsStore.get<unknown>(BACKGROUND_STYLE_KEY)
  return isTaskbarBackgroundStyle(value) ? value : DEFAULT_TASKBAR_BACKGROUND_STYLE
}

/** 保存背景样式并同步任务栏窗口。 */
async function saveTaskbarBackgroundStyle(style: TaskbarBackgroundStyle): Promise<void> {
  await settingsStore.set(BACKGROUND_STYLE_KEY, style)
  await emit(BACKGROUND_STYLE_CHANGED_EVENT, style)
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
  const replacedBackground =
    style === 'vertical-gradient' && previousBackground !== FULL_HEIGHT_PROGRESS_BACKGROUND_STYLE
  if (replacedBackground) {
    await saveTaskbarBackgroundStyle(FULL_HEIGHT_PROGRESS_BACKGROUND_STYLE)
  }
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
