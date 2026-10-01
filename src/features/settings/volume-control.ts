import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

const VOLUME_CONTROL_TARGET_KEY = 'taskbar.controls.volumeTarget'
const VOLUME_CONTROL_TARGET_CHANGED_EVENT = 'settings://volume-control-target-changed'

export const VOLUME_CONTROL_TARGETS = ['application', 'system'] as const
export type VolumeControlTarget = (typeof VOLUME_CONTROL_TARGETS)[number]

export const DEFAULT_VOLUME_CONTROL_TARGET: VolumeControlTarget = 'system'

/** 判断外部值是否为受支持的音量控制对象。 */
export function isVolumeControlTarget(value: unknown): value is VolumeControlTarget {
  return VOLUME_CONTROL_TARGETS.some((target) => target === value)
}

/** 读取音量控制对象，缺失或损坏时回退到默认的系统音量。 */
export async function getVolumeControlTarget(): Promise<VolumeControlTarget> {
  const value = await settingsStore.get<unknown>(VOLUME_CONTROL_TARGET_KEY)
  return isVolumeControlTarget(value) ? value : DEFAULT_VOLUME_CONTROL_TARGET
}

/** 持久化音量控制对象，并通知任务栏及设置页中的音量控件。 */
export async function setVolumeControlTarget(target: VolumeControlTarget): Promise<void> {
  await settingsStore.set(VOLUME_CONTROL_TARGET_KEY, target)
  await emit(VOLUME_CONTROL_TARGET_CHANGED_EVENT, target)
}

/** 监听音量控制对象变化，并在事件边界规范化外部值。 */
export async function listenVolumeControlTargetChange(
  handler: (target: VolumeControlTarget) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(VOLUME_CONTROL_TARGET_CHANGED_EVENT, ({ payload }) => {
    handler(isVolumeControlTarget(payload) ? payload : DEFAULT_VOLUME_CONTROL_TARGET)
  })
}
