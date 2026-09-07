import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

const TASKBAR_AUTO_HIDE_KEY = 'taskbar.autoHide'
const TASKBAR_AUTO_HIDE_CHANGED_EVENT = 'settings://taskbar-auto-hide-changed'

export interface TaskbarAutoHide {
  whenPaused: boolean
  whenNoMediaSession: boolean
}

export const DEFAULT_TASKBAR_AUTO_HIDE: TaskbarAutoHide = {
  whenPaused: false,
  whenNoMediaSession: false,
}

/** 将外部数据规范为完整的 bar 自动隐藏配置。 */
export function normalizeTaskbarAutoHide(value: unknown): TaskbarAutoHide {
  const record =
    typeof value === 'object' && value !== null
      ? (value as Partial<Record<keyof TaskbarAutoHide, unknown>>)
      : {}
  return {
    whenPaused:
      typeof record.whenPaused === 'boolean'
        ? record.whenPaused
        : DEFAULT_TASKBAR_AUTO_HIDE.whenPaused,
    whenNoMediaSession:
      typeof record.whenNoMediaSession === 'boolean'
        ? record.whenNoMediaSession
        : DEFAULT_TASKBAR_AUTO_HIDE.whenNoMediaSession,
  }
}

/** 读取 bar 自动隐藏配置。 */
export async function getTaskbarAutoHide(): Promise<TaskbarAutoHide> {
  return normalizeTaskbarAutoHide(await settingsStore.get<unknown>(TASKBAR_AUTO_HIDE_KEY))
}

/** 持久化 bar 自动隐藏配置，并通知全部任务栏窗口。 */
export async function setTaskbarAutoHide(value: TaskbarAutoHide): Promise<void> {
  const normalized = normalizeTaskbarAutoHide(value)
  await settingsStore.set(TASKBAR_AUTO_HIDE_KEY, normalized)
  await emit(TASKBAR_AUTO_HIDE_CHANGED_EVENT, normalized)
}

/** 监听 bar 自动隐藏配置变更。 */
export async function listenTaskbarAutoHideChange(
  handler: (value: TaskbarAutoHide) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_AUTO_HIDE_CHANGED_EVENT, ({ payload }) => {
    handler(normalizeTaskbarAutoHide(payload))
  })
}
