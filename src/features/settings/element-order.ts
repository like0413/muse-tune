import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

export const TASKBAR_ELEMENTS = ['cover', 'track-info', 'controls'] as const
const TASKBAR_ELEMENT_ORDER_KEY = 'taskbar.elementOrder'
const TASKBAR_ELEMENT_ORDER_CHANGED_EVENT = 'settings://taskbar-element-order-changed'

export type TaskbarElement = (typeof TASKBAR_ELEMENTS)[number]

export const DEFAULT_TASKBAR_ELEMENT_ORDER: TaskbarElement[] = [...TASKBAR_ELEMENTS]

/** 判断外部值是否为包含全部区块且没有重复项的有效排列。 */
export function isTaskbarElementOrder(value: unknown): value is TaskbarElement[] {
  if (!Array.isArray(value) || value.length !== TASKBAR_ELEMENTS.length) return false

  const elements = new Set(value)
  return (
    elements.size === TASKBAR_ELEMENTS.length &&
    TASKBAR_ELEMENTS.every((element) => elements.has(element))
  )
}

/** 读取任务栏区块顺序；缺失或损坏时恢复默认排列。 */
export async function getTaskbarElementOrder(): Promise<TaskbarElement[]> {
  const order = await settingsStore.get<unknown>(TASKBAR_ELEMENT_ORDER_KEY)
  return isTaskbarElementOrder(order) ? [...order] : [...DEFAULT_TASKBAR_ELEMENT_ORDER]
}

/** 持久化区块顺序，并通知全部任务栏窗口立即更新。 */
export async function setTaskbarElementOrder(order: TaskbarElement[]): Promise<void> {
  if (!isTaskbarElementOrder(order)) throw new Error('无效的任务栏区块顺序')

  const nextOrder = [...order]
  await settingsStore.set(TASKBAR_ELEMENT_ORDER_KEY, nextOrder)
  await emit(TASKBAR_ELEMENT_ORDER_CHANGED_EVENT, nextOrder)
}

/** 监听设置窗口发出的区块顺序变更。 */
export async function listenTaskbarElementOrderChange(
  handler: (order: TaskbarElement[]) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_ELEMENT_ORDER_CHANGED_EVENT, ({ payload }) => {
    if (isTaskbarElementOrder(payload)) handler([...payload])
  })
}
