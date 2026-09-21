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

export async function listTaskbarDisplays(): Promise<TaskbarDisplay[]> {
  return listNativeTaskbarDisplays()
}

/**
 * 立即切换目标显示器，并在成功后持久化选择。
 *
 * 必须经由本函数写入：原生侧只在启动时读取该键（写入契约见 `defaults.ts`）。
 */
export async function setTaskbarDisplayTarget(target: string): Promise<void> {
  await applyTaskbarDisplayTarget(target)
  await settingsStore.set(TASKBAR_DISPLAY_TARGET_KEY, target)
}

export async function applyTaskbarDisplayTarget(target: string): Promise<void> {
  await applyNativeTaskbarDisplayTarget(target)
}
