import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

const TASKBAR_CONTROLS_VISIBILITY_KEY = 'taskbar.controls.visibility'
const TASKBAR_CONTROLS_VISIBILITY_CHANGED_EVENT = 'settings://taskbar-controls-visibility-changed'

export interface TaskbarControlsVisibility {
  visible: boolean
  previous: boolean
  playPause: boolean
  next: boolean
  volume: boolean
}

export const DEFAULT_TASKBAR_CONTROLS_VISIBILITY: TaskbarControlsVisibility = {
  visible: true,
  previous: true,
  playPause: true,
  next: true,
  volume: true,
}

/** 将外部值规范为完整控制按钮配置，损坏字段单独回退默认值。 */
export function normalizeTaskbarControlsVisibility(value: unknown): TaskbarControlsVisibility {
  const candidate = typeof value === 'object' && value !== null ? value : {}
  const record = candidate as Partial<Record<keyof TaskbarControlsVisibility, unknown>>
  return {
    visible:
      typeof record.visible === 'boolean'
        ? record.visible
        : DEFAULT_TASKBAR_CONTROLS_VISIBILITY.visible,
    previous:
      typeof record.previous === 'boolean'
        ? record.previous
        : DEFAULT_TASKBAR_CONTROLS_VISIBILITY.previous,
    playPause:
      typeof record.playPause === 'boolean'
        ? record.playPause
        : DEFAULT_TASKBAR_CONTROLS_VISIBILITY.playPause,
    next: typeof record.next === 'boolean' ? record.next : DEFAULT_TASKBAR_CONTROLS_VISIBILITY.next,
    volume:
      typeof record.volume === 'boolean'
        ? record.volume
        : DEFAULT_TASKBAR_CONTROLS_VISIBILITY.volume,
  }
}

/** 读取控制按钮显示配置。 */
export async function getTaskbarControlsVisibility(): Promise<TaskbarControlsVisibility> {
  return normalizeTaskbarControlsVisibility(
    await settingsStore.get<unknown>(TASKBAR_CONTROLS_VISIBILITY_KEY),
  )
}

/** 持久化完整控制按钮配置，并通知全部任务栏窗口。 */
export async function setTaskbarControlsVisibility(
  visibility: TaskbarControlsVisibility,
): Promise<void> {
  const normalized = normalizeTaskbarControlsVisibility(visibility)
  await settingsStore.set(TASKBAR_CONTROLS_VISIBILITY_KEY, normalized)
  await emit(TASKBAR_CONTROLS_VISIBILITY_CHANGED_EVENT, normalized)
}

/** 监听控制按钮显示配置变化。 */
export async function listenTaskbarControlsVisibilityChange(
  handler: (visibility: TaskbarControlsVisibility) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_CONTROLS_VISIBILITY_CHANGED_EVENT, ({ payload }) => {
    handler(normalizeTaskbarControlsVisibility(payload))
  })
}
