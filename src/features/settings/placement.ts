import { setTaskbarPlacement as applyNativeTaskbarPlacement } from '@/features/taskbar/client'
import { TASKBAR_PLACEMENTS, type TaskbarPlacement } from '@/features/taskbar/contracts'

import { settingsStore } from './store'

export type { TaskbarPlacement } from '@/features/taskbar/contracts'

const TASKBAR_PLACEMENT_KEY = 'taskbar.placement'
const DEFAULT_TASKBAR_PLACEMENT: TaskbarPlacement = 'auto'

/** 判断持久化值是否为受支持的播放器位置。 */
export function isTaskbarPlacement(value: unknown): value is TaskbarPlacement {
  return typeof value === 'string' && TASKBAR_PLACEMENTS.some((placement) => placement === value)
}

/** 读取播放器位置，缺失或损坏时回退到自动模式。 */
export async function getTaskbarPlacement(): Promise<TaskbarPlacement> {
  const placement = await settingsStore.get<unknown>(TASKBAR_PLACEMENT_KEY)
  return isTaskbarPlacement(placement) ? placement : DEFAULT_TASKBAR_PLACEMENT
}

/** 立即应用播放器位置，并在成功后持久化选择。 */
export async function setTaskbarPlacement(placement: TaskbarPlacement): Promise<void> {
  await applyTaskbarPlacement(placement)
  await settingsStore.set(TASKBAR_PLACEMENT_KEY, placement)
}

/** 将播放器位置同步到原生任务栏定位线程。 */
export async function applyTaskbarPlacement(placement: TaskbarPlacement): Promise<void> {
  await applyNativeTaskbarPlacement(placement)
}
