import { setTaskbarWidth as applyNativeTaskbarWidth } from '@/features/taskbar/client'
import { TASKBAR_WIDTH_MODES, type TaskbarWidthMode } from '@/features/taskbar/contracts'

import {
  DEFAULT_TASKBAR_WIDTH,
  DEFAULT_TASKBAR_WIDTH_MODE,
  TASKBAR_WIDTH_MAX,
  TASKBAR_WIDTH_MIN,
} from './defaults'
import { normalizeIntegerInRange } from './normalize'
import { settingsStore } from './store'

export type { TaskbarWidthMode } from '@/features/taskbar/contracts'

/** 可调范围与原生侧共用同一份取值，见 defaults.ts 的 native-defaults.json 说明。 */
export { TASKBAR_WIDTH_MAX, TASKBAR_WIDTH_MIN }

export const TASKBAR_WIDTH_PRESETS = {
  compact: 200,
  standard: 250,
  wide: 360,
} as const

export type TaskbarWidthPreset = keyof typeof TASKBAR_WIDTH_PRESETS | 'custom'

const TASKBAR_WIDTH_KEY = 'taskbar.width'
const TASKBAR_WIDTH_MODE_KEY = 'taskbar.widthMode'

export function getTaskbarWidthPreset(width: number): TaskbarWidthPreset {
  const preset = Object.entries(TASKBAR_WIDTH_PRESETS).find(([, value]) => value === width)?.[0]
  return (preset as keyof typeof TASKBAR_WIDTH_PRESETS | undefined) ?? 'custom'
}

/** 将外部宽度值规范到受支持的整数 DIP 范围。 */
export function normalizeTaskbarWidth(value: unknown): number | undefined {
  return normalizeIntegerInRange(value, TASKBAR_WIDTH_MIN, TASKBAR_WIDTH_MAX)
}

export function isTaskbarWidthMode(value: unknown): value is TaskbarWidthMode {
  return TASKBAR_WIDTH_MODES.some((mode) => mode === value)
}

export async function getTaskbarWidth(): Promise<number> {
  const width = normalizeTaskbarWidth(await settingsStore.get<unknown>(TASKBAR_WIDTH_KEY))
  return width ?? DEFAULT_TASKBAR_WIDTH
}

export async function getTaskbarWidthMode(): Promise<TaskbarWidthMode> {
  const mode = await settingsStore.get<unknown>(TASKBAR_WIDTH_MODE_KEY)
  return isTaskbarWidthMode(mode) ? mode : DEFAULT_TASKBAR_WIDTH_MODE
}

export async function applyTaskbarWidth(width: number, mode: TaskbarWidthMode): Promise<void> {
  const normalized = normalizeTaskbarWidth(width)
  if (normalized !== undefined) {
    await applyNativeTaskbarWidth(normalized, mode)
  }
}

/**
 * 立即应用宽度模式与基准宽度，并在成功后持久化。
 *
 * 必须经由本函数写入：原生侧只在启动时读取这两个键（写入契约见 `defaults.ts`）。
 */
export async function setTaskbarWidth(width: number, mode: TaskbarWidthMode): Promise<void> {
  const normalized = normalizeTaskbarWidth(width)
  if (normalized === undefined) {
    return
  }

  await applyTaskbarWidth(normalized, mode)
  await settingsStore.set(TASKBAR_WIDTH_KEY, normalized)
  await settingsStore.set(TASKBAR_WIDTH_MODE_KEY, mode)
}
