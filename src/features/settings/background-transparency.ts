import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY } from './defaults'
import { normalizeIntegerInRange } from './normalize'
import { SETTINGS_SCHEMA_VERSIONS } from './storage/schema-versions'
import { loadVersionedSetting, setVersionedSetting } from './storage/versioned-setting'

export const TASKBAR_TRANSPARENCY_MIN = 0
export const TASKBAR_TRANSPARENCY_MAX = 100

const TASKBAR_BACKGROUND_TRANSPARENCY_KEY = 'taskbar.backgroundTransparency'
const TASKBAR_BACKGROUND_TRANSPARENCY_CHANGED_EVENT =
  'settings://taskbar-background-transparency-changed'

/** 将外部透明度值规范到 0～100 的整数范围。 */
export function normalizeTaskbarBackgroundTransparency(value: unknown): number | undefined {
  return normalizeIntegerInRange(value, TASKBAR_TRANSPARENCY_MIN, TASKBAR_TRANSPARENCY_MAX)
}

const backgroundTransparencyStorage = {
  key: TASKBAR_BACKGROUND_TRANSPARENCY_KEY,
  version: SETTINGS_SCHEMA_VERSIONS.taskbar.backgroundTransparency,
  defaultValue: DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY,
  normalize: (value: unknown) =>
    normalizeTaskbarBackgroundTransparency(value) ?? DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY,
}

/** 读取背景透明度；版本不兼容时仅恢复此项默认值。 */
export async function getTaskbarBackgroundTransparency(): Promise<number> {
  return loadVersionedSetting(backgroundTransparencyStorage)
}

/** 仅预览背景透明度，不触发持久化写入。 */
export async function previewTaskbarBackgroundTransparency(transparency: number): Promise<void> {
  const normalized = normalizeTaskbarBackgroundTransparency(transparency)
  if (normalized !== undefined) {
    await emit(TASKBAR_BACKGROUND_TRANSPARENCY_CHANGED_EVENT, normalized)
  }
}

/** 持久化背景透明度，并通知已运行的任务栏窗口应用最终值。 */
export async function setTaskbarBackgroundTransparency(transparency: number): Promise<void> {
  const normalized = normalizeTaskbarBackgroundTransparency(transparency)
  if (normalized === undefined) {
    return
  }

  const saved = await setVersionedSetting(backgroundTransparencyStorage, normalized)
  await emit(TASKBAR_BACKGROUND_TRANSPARENCY_CHANGED_EVENT, saved)
}

/** 监听设置窗口发出的任务栏背景透明度变更。 */
export async function listenTaskbarBackgroundTransparencyChange(
  handler: (transparency: number) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_BACKGROUND_TRANSPARENCY_CHANGED_EVENT, ({ payload }) => {
    const transparency = normalizeTaskbarBackgroundTransparency(payload)
    if (transparency !== undefined) {
      handler(transparency)
    }
  })
}
