import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

const TASKBAR_CONTROLS_VISIBILITY_KEY = 'taskbar.controls.visibility'
const TASKBAR_CONTROLS_VISIBILITY_CHANGED_EVENT = 'settings://taskbar-controls-visibility-changed'
export const TASKBAR_CONTROL_BUTTONS = ['previous', 'playPause', 'next'] as const

export type TaskbarControlButton = (typeof TASKBAR_CONTROL_BUTTONS)[number]

export interface TaskbarControlsVisibility {
  visible: boolean
  previous: boolean
  playPause: boolean
  next: boolean
  order: TaskbarControlButton[]
}

/** order 由 TASKBAR_CONTROL_BUTTONS 派生，故整个默认值留在本模块，未收进 defaults.ts。 */
export const DEFAULT_TASKBAR_CONTROLS_VISIBILITY: TaskbarControlsVisibility = {
  visible: true,
  previous: true,
  playPause: true,
  next: true,
  order: [...TASKBAR_CONTROL_BUTTONS],
}

/** 规范按钮顺序，并从旧配置中移除已经取消的音量按钮。 */
function normalizeTaskbarControlButtonOrder(value: unknown): TaskbarControlButton[] {
  if (!Array.isArray(value)) return [...DEFAULT_TASKBAR_CONTROLS_VISIBILITY.order]

  const migrated = value.filter((button) => button !== 'volume')
  const buttons = new Set(migrated)
  if (
    migrated.length !== TASKBAR_CONTROL_BUTTONS.length ||
    buttons.size !== TASKBAR_CONTROL_BUTTONS.length ||
    !TASKBAR_CONTROL_BUTTONS.every((button) => buttons.has(button))
  ) {
    return [...DEFAULT_TASKBAR_CONTROLS_VISIBILITY.order]
  }

  return migrated as TaskbarControlButton[]
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
    order: normalizeTaskbarControlButtonOrder(record.order),
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
