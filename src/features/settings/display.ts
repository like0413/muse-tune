import { invoke } from '@tauri-apps/api/core'

import { settingsStore } from './store'

export const ALL_TASKBAR_DISPLAYS = 'all'

export interface TaskbarDisplay {
  id: string
  label: string
  width: number
  height: number
  isPrimary: boolean
}

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
  return invoke<TaskbarDisplay[]>('list_taskbar_displays')
}

/** 立即切换目标显示器，并在成功后持久化选择。 */
export async function setTaskbarDisplayTarget(target: string): Promise<void> {
  await applyTaskbarDisplayTarget(target)
  await settingsStore.set(TASKBAR_DISPLAY_TARGET_KEY, target)
}

/** 将目标显示器同步到原生多任务栏窗口管理线程。 */
export async function applyTaskbarDisplayTarget(target: string): Promise<void> {
  await invoke('set_taskbar_display_target', { target })
}
