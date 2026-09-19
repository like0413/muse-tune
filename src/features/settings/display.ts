import {
  setTaskbarDisplayTarget as applyNativeTaskbarDisplayTarget,
  listTaskbarDisplays as listNativeTaskbarDisplays,
} from '@/features/taskbar/client'
import type { TaskbarDisplay } from '@/features/taskbar/contracts'

import { ALL_TASKBAR_DISPLAYS } from './defaults'
import { settingsStore } from './store'

export type { TaskbarDisplay } from '@/features/taskbar/contracts'
export { ALL_TASKBAR_DISPLAYS }

const TASKBAR_DISPLAY_TARGET_KEY = 'taskbar.displayTarget'

/** 读取目标显示器，缺失时默认在全部任务栏显示。 */
export async function getTaskbarDisplayTarget(): Promise<string> {
  const target = await settingsStore.get<unknown>(TASKBAR_DISPLAY_TARGET_KEY)
  return typeof target === 'string' && target.length > 0 && target.trim() === target
    ? target
    : ALL_TASKBAR_DISPLAYS
}

/** 枚举当前由 Windows 创建了任务栏的显示器。 */
export async function listTaskbarDisplays(): Promise<TaskbarDisplay[]> {
  return listNativeTaskbarDisplays()
}

/**
 * 立即切换目标显示器，并在成功后持久化选择。
 *
 * 必须经由本函数写入：原生侧只在启动时读取存储，运行期间不再同步该键。
 * 绕过它直接写 `settingsStore` 会让原生保持旧值，而设置界面显示新值（见 `defaults.ts` 的写入契约）。
 */
export async function setTaskbarDisplayTarget(target: string): Promise<void> {
  await applyTaskbarDisplayTarget(target)
  await settingsStore.set(TASKBAR_DISPLAY_TARGET_KEY, target)
}

/** 将目标显示器同步到原生多任务栏窗口管理线程。 */
export async function applyTaskbarDisplayTarget(target: string): Promise<void> {
  await applyNativeTaskbarDisplayTarget(target)
}
