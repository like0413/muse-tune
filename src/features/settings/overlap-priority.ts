import { setTaskbarOverlapPriority as applyNativeTaskbarOverlapPriority } from '@/features/taskbar/client'
import {
  TASKBAR_OVERLAP_PRIORITIES,
  type TaskbarOverlapPriority,
} from '@/features/taskbar/contracts'

import { DEFAULT_TASKBAR_OVERLAP_PRIORITY } from './defaults'
import { settingsStore } from './store'

export type { TaskbarOverlapPriority } from '@/features/taskbar/contracts'

const TASKBAR_OVERLAP_PRIORITY_KEY = 'taskbar.overlapPriority'

/** 判断持久化值是否为受支持的任务栏元素遮挡优先级。 */
export function isTaskbarOverlapPriority(value: unknown): value is TaskbarOverlapPriority {
  return (
    typeof value === 'string' && TASKBAR_OVERLAP_PRIORITIES.some((priority) => priority === value)
  )
}

/** 读取任务栏元素遮挡优先级，缺失或损坏时维持播放器优先。 */
export async function getTaskbarOverlapPriority(): Promise<TaskbarOverlapPriority> {
  const priority = await settingsStore.get<unknown>(TASKBAR_OVERLAP_PRIORITY_KEY)
  return isTaskbarOverlapPriority(priority) ? priority : DEFAULT_TASKBAR_OVERLAP_PRIORITY
}

/**
 * 立即应用任务栏元素遮挡优先级，并在成功后持久化选择。
 *
 * 必须经由本函数写入：原生侧只在启动时读取存储，运行期间不再同步该键。
 * 绕过它直接写 `settingsStore` 会让原生保持旧值，而设置界面显示新值（见 `defaults.ts` 的写入契约）。
 */
export async function setTaskbarOverlapPriority(priority: TaskbarOverlapPriority): Promise<void> {
  await applyTaskbarOverlapPriority(priority)
  await settingsStore.set(TASKBAR_OVERLAP_PRIORITY_KEY, priority)
}

/** 将任务栏元素遮挡优先级同步到原生任务栏定位线程。 */
export async function applyTaskbarOverlapPriority(priority: TaskbarOverlapPriority): Promise<void> {
  await applyNativeTaskbarOverlapPriority(priority)
}
