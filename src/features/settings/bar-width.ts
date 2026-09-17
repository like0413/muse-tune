import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'
import { clamp } from 'es-toolkit'

import { setTaskbarWidth as applyNativeTaskbarWidth } from '@/features/taskbar/client'

import { settingsStore } from './store'

export const TASKBAR_WIDTH_MIN = 200
export const TASKBAR_WIDTH_MAX = 360
export const TASKBAR_WIDTH_PRESETS = {
  compact: 200,
  standard: 250,
  wide: 360,
} as const

export type TaskbarWidthPreset = keyof typeof TASKBAR_WIDTH_PRESETS | 'custom'

const TASKBAR_WIDTH_KEY = 'taskbar.width'
const TASKBAR_WIDTH_CHANGED_EVENT = 'settings://taskbar-width-changed'
const DEFAULT_TASKBAR_WIDTH = TASKBAR_WIDTH_MAX

/** 根据已保存宽度还原预设；非精确预设值归入自由调整。 */
export function getTaskbarWidthPreset(width: number): TaskbarWidthPreset {
  const preset = Object.entries(TASKBAR_WIDTH_PRESETS).find(([, value]) => value === width)?.[0]
  return (preset as keyof typeof TASKBAR_WIDTH_PRESETS | undefined) ?? 'custom'
}

/** 将外部宽度值规范到受支持的整数 DIP 范围。 */
export function normalizeTaskbarWidth(value: unknown): number | undefined {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return undefined
  }

  return Math.round(clamp(value, TASKBAR_WIDTH_MIN, TASKBAR_WIDTH_MAX))
}

/** 读取 bar 基准宽度，缺失或损坏时使用当前的 360 DIP。 */
export async function getTaskbarWidth(): Promise<number> {
  const width = normalizeTaskbarWidth(await settingsStore.get<unknown>(TASKBAR_WIDTH_KEY))
  return width ?? DEFAULT_TASKBAR_WIDTH
}

/** 将宽度应用到原生任务栏窗口，并通知 bar 同步响应式布局。 */
export async function applyTaskbarWidth(width: number): Promise<void> {
  const normalized = normalizeTaskbarWidth(width)
  if (normalized !== undefined) {
    await applyNativeTaskbarWidth(normalized)
    await emit(TASKBAR_WIDTH_CHANGED_EVENT, normalized)
  }
}

/** 监听设置窗口发出的 bar 宽度变化，包括拖动预览与回滚。 */
export async function listenTaskbarWidthChange(
  handler: (width: number) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_WIDTH_CHANGED_EVENT, ({ payload }) => {
    const width = normalizeTaskbarWidth(payload)
    if (width !== undefined) {
      handler(width)
    }
  })
}

/** 立即应用 bar 宽度，并在成功后持久化。 */
export async function setTaskbarWidth(width: number): Promise<void> {
  const normalized = normalizeTaskbarWidth(width)
  if (normalized === undefined) {
    return
  }

  await applyTaskbarWidth(normalized)
  await settingsStore.set(TASKBAR_WIDTH_KEY, normalized)
}
