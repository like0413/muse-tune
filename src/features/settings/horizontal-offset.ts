import { setTaskbarHorizontalOffset as applyNativeTaskbarHorizontalOffset } from '@/features/taskbar/client'

import {
  DEFAULT_TASKBAR_HORIZONTAL_OFFSET,
  TASKBAR_HORIZONTAL_OFFSET_MAX,
  TASKBAR_HORIZONTAL_OFFSET_MIN,
} from './defaults'
import { normalizeIntegerInRange } from './normalize'
import { settingsStore } from './store'

const TASKBAR_HORIZONTAL_OFFSET_KEY = 'taskbar.horizontalOffset'

export { TASKBAR_HORIZONTAL_OFFSET_MAX, TASKBAR_HORIZONTAL_OFFSET_MIN }

/** 将外部值规范为受支持的整数 DIP 水平偏移。 */
export function normalizeTaskbarHorizontalOffset(value: unknown): number | undefined {
  return normalizeIntegerInRange(
    value,
    TASKBAR_HORIZONTAL_OFFSET_MIN,
    TASKBAR_HORIZONTAL_OFFSET_MAX,
  )
}

/** 读取水平偏移；缺失或损坏时回退到零偏移。 */
export async function getTaskbarHorizontalOffset(): Promise<number> {
  const offset = normalizeTaskbarHorizontalOffset(
    await settingsStore.get<unknown>(TASKBAR_HORIZONTAL_OFFSET_KEY),
  )
  return offset ?? DEFAULT_TASKBAR_HORIZONTAL_OFFSET
}

/** 只应用水平偏移，不写入持久化存储，用于保存失败后的原生状态回滚。 */
export async function applyTaskbarHorizontalOffset(offset: number): Promise<void> {
  const normalized = normalizeTaskbarHorizontalOffset(offset)
  if (normalized !== undefined) {
    await applyNativeTaskbarHorizontalOffset(normalized)
  }
}

/** 立即应用水平偏移，并在成功后持久化。 */
export async function setTaskbarHorizontalOffset(offset: number): Promise<void> {
  const normalized = normalizeTaskbarHorizontalOffset(offset)
  if (normalized === undefined) return

  await applyTaskbarHorizontalOffset(normalized)
  await settingsStore.set(TASKBAR_HORIZONTAL_OFFSET_KEY, normalized)
}
