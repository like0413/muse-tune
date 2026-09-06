import { invoke } from '@tauri-apps/api/core'
import { clamp } from 'es-toolkit'

import { settingsStore } from './store'

export const TASKBAR_WIDTH_MIN = 200
export const TASKBAR_WIDTH_MAX = 360

const TASKBAR_WIDTH_KEY = 'taskbar.width'
const DEFAULT_TASKBAR_WIDTH = TASKBAR_WIDTH_MAX

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

/** 仅将宽度应用到原生任务栏窗口，不写入持久化存储。 */
export async function applyTaskbarWidth(width: number): Promise<void> {
  const normalized = normalizeTaskbarWidth(width)
  if (normalized !== undefined) {
    await invoke('set_taskbar_width', { width: normalized })
  }
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
