import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

export const TASKBAR_ELEMENTS = ['cover', 'track-info', 'controls'] as const
export const TASKBAR_LYRICS_ELEMENTS = ['cover', 'lyrics'] as const
const TASKBAR_ELEMENT_ORDER_KEY = 'taskbar.elementOrder'
const TASKBAR_ELEMENT_ORDER_CHANGED_EVENT = 'settings://taskbar-element-order-changed'

export type TaskbarElement = (typeof TASKBAR_ELEMENTS)[number]
export type TaskbarLyricsElement = (typeof TASKBAR_LYRICS_ELEMENTS)[number]

export interface TaskbarElementOrder {
  normal: TaskbarElement[]
  lyrics: TaskbarLyricsElement[]
}

/** 默认顺序由元素清单派生，故留在本模块，未收进 defaults.ts。 */
export const DEFAULT_TASKBAR_ELEMENT_ORDER: TaskbarElementOrder = {
  normal: [...TASKBAR_ELEMENTS],
  lyrics: [...TASKBAR_LYRICS_ELEMENTS],
}

/** 判断数组是否完整包含指定元素且没有重复项。 */
function isCompleteOrder<T extends string>(value: unknown, elements: readonly T[]): value is T[] {
  if (!Array.isArray(value) || value.length !== elements.length) return false

  const values = new Set(value)
  return values.size === elements.length && elements.every((element) => values.has(element))
}

/** 判断外部值是否为两种模式的完整元素顺序。 */
export function isTaskbarElementOrder(value: unknown): value is TaskbarElementOrder {
  if (typeof value !== 'object' || value === null) return false

  const order = value as Partial<TaskbarElementOrder>
  return (
    isCompleteOrder(order.normal, TASKBAR_ELEMENTS) &&
    isCompleteOrder(order.lyrics, TASKBAR_LYRICS_ELEMENTS)
  )
}

/** 返回不共享数组引用的顺序副本。 */
export function cloneTaskbarElementOrder(order: TaskbarElementOrder): TaskbarElementOrder {
  return { normal: [...order.normal], lyrics: [...order.lyrics] }
}

/** 兼容旧版仅保存普通模式数组的设置。 */
export function normalizeTaskbarElementOrder(value: unknown): TaskbarElementOrder {
  if (isTaskbarElementOrder(value)) return cloneTaskbarElementOrder(value)
  if (isCompleteOrder(value, TASKBAR_ELEMENTS)) {
    return { normal: [...value], lyrics: [...DEFAULT_TASKBAR_ELEMENT_ORDER.lyrics] }
  }
  return cloneTaskbarElementOrder(DEFAULT_TASKBAR_ELEMENT_ORDER)
}

export async function getTaskbarElementOrder(): Promise<TaskbarElementOrder> {
  return normalizeTaskbarElementOrder(await settingsStore.get<unknown>(TASKBAR_ELEMENT_ORDER_KEY))
}

export async function setTaskbarElementOrder(order: TaskbarElementOrder): Promise<void> {
  if (!isTaskbarElementOrder(order)) throw new Error('无效的任务栏区块顺序')

  const nextOrder = cloneTaskbarElementOrder(order)
  await settingsStore.set(TASKBAR_ELEMENT_ORDER_KEY, nextOrder)
  await emit(TASKBAR_ELEMENT_ORDER_CHANGED_EVENT, nextOrder)
}

export async function listenTaskbarElementOrderChange(
  handler: (order: TaskbarElementOrder) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_ELEMENT_ORDER_CHANGED_EVENT, ({ payload }) => {
    if (isTaskbarElementOrder(payload)) handler(cloneTaskbarElementOrder(payload))
  })
}
