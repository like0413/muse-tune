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

/**
 * 宽度档位：standard 与 wide 直接引用单一数据源（native-defaults.json）导出的常量，
 * 随默认宽度/可调上限调整自动跟随；compact 为自定义紧凑档（介于 widthMin 与 standard 之间），
 * 无 JSON 锚点，故保留字面量并注释说明。
 */
export const TASKBAR_WIDTH_PRESETS = {
  compact: 170,
  standard: DEFAULT_TASKBAR_WIDTH,
  wide: TASKBAR_WIDTH_MAX,
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

  // normalized 已在上方规范化（有限、取整、钳制到区间），此处直接推送已规范化的值，
  // 避免经 applyTaskbarWidth 再 normalize 一次（那也会对预览路径做同样校验）。
  await applyNativeTaskbarWidth(normalized, mode)
  await settingsStore.set(TASKBAR_WIDTH_KEY, normalized)
  await settingsStore.set(TASKBAR_WIDTH_MODE_KEY, mode)
}
