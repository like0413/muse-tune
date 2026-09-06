import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

const TASKBAR_PROGRESS_STYLES = ['bottom', 'vertical-gradient'] as const
const TASKBAR_PROGRESS_STYLE_KEY = 'taskbar.progressStyle'
const TASKBAR_PROGRESS_STYLE_CHANGED_EVENT = 'settings://taskbar-progress-style-changed'

export type TaskbarProgressStyle = (typeof TASKBAR_PROGRESS_STYLES)[number]

export const DEFAULT_TASKBAR_PROGRESS_STYLE: TaskbarProgressStyle = 'bottom'

/** 判断外部值是否为受支持的播放进度样式。 */
export function isTaskbarProgressStyle(value: unknown): value is TaskbarProgressStyle {
  return typeof value === 'string' && TASKBAR_PROGRESS_STYLES.some((style) => style === value)
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
