import { clamp } from 'es-toolkit'

import { setTaskbarWidth as applyNativeTaskbarWidth } from '@/features/taskbar/client'
import { TASKBAR_WIDTH_MODES, type TaskbarWidthMode } from '@/features/taskbar/contracts'

import { settingsStore } from './store'

export type { TaskbarWidthMode } from '@/features/taskbar/contracts'

export const TASKBAR_WIDTH_MIN = 200
export const TASKBAR_WIDTH_MAX = 360
export const TASKBAR_WIDTH_PRESETS = {
  compact: 200,
  standard: 250,
  wide: 360,
} as const

export type TaskbarWidthPreset = keyof typeof TASKBAR_WIDTH_PRESETS | 'custom'

const TASKBAR_WIDTH_KEY = 'taskbar.width'
const TASKBAR_WIDTH_MODE_KEY = 'taskbar.widthMode'
const DEFAULT_TASKBAR_WIDTH = TASKBAR_WIDTH_MAX
const DEFAULT_TASKBAR_WIDTH_MODE: TaskbarWidthMode = 'fixed'

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

/** 判断持久化值是否为受支持的宽度模式。 */
export function isTaskbarWidthMode(value: unknown): value is TaskbarWidthMode {
  return TASKBAR_WIDTH_MODES.some((mode) => mode === value)
}

/** 读取 bar 基准宽度，缺失或损坏时使用当前的 360 DIP。 */
export async function getTaskbarWidth(): Promise<number> {
  const width = normalizeTaskbarWidth(await settingsStore.get<unknown>(TASKBAR_WIDTH_KEY))
  return width ?? DEFAULT_TASKBAR_WIDTH
}

/** 读取 bar 宽度模式，缺失或损坏时使用固定宽度。 */
export async function getTaskbarWidthMode(): Promise<TaskbarWidthMode> {
  const mode = await settingsStore.get<unknown>(TASKBAR_WIDTH_MODE_KEY)
  return isTaskbarWidthMode(mode) ? mode : DEFAULT_TASKBAR_WIDTH_MODE
}

/** 将宽度模式与基准宽度一起应用到原生任务栏窗口。 */
export async function applyTaskbarWidth(width: number, mode: TaskbarWidthMode): Promise<void> {
  const normalized = normalizeTaskbarWidth(width)
  if (normalized !== undefined) {
    await applyNativeTaskbarWidth(normalized, mode)
  }
}

/** 立即应用宽度模式与基准宽度，并在成功后持久化。 */
export async function setTaskbarWidth(width: number, mode: TaskbarWidthMode): Promise<void> {
  const normalized = normalizeTaskbarWidth(width)
  if (normalized === undefined) {
    return
  }

  await applyTaskbarWidth(normalized, mode)
  await settingsStore.set(TASKBAR_WIDTH_KEY, normalized)
  await settingsStore.set(TASKBAR_WIDTH_MODE_KEY, mode)
}
