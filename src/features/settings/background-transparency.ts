import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'
import { clamp } from 'es-toolkit'

import { settingsStore } from './store'

export const TASKBAR_TRANSPARENCY_MIN = 0
export const TASKBAR_TRANSPARENCY_MAX = 100

const LEGACY_TASKBAR_MATERIAL_KEY = 'taskbar.material'
const TASKBAR_BACKGROUND_TRANSPARENCY_KEY = 'taskbar.backgroundTransparency'
const TASKBAR_BACKGROUND_TRANSPARENCY_CHANGED_EVENT =
  'settings://taskbar-background-transparency-changed'
const DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY = 0

/** 将外部透明度值规范到 0～100 的整数范围。 */
export function normalizeTaskbarBackgroundTransparency(value: unknown): number | undefined {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return undefined
  }

  return Math.round(clamp(value, TASKBAR_TRANSPARENCY_MIN, TASKBAR_TRANSPARENCY_MAX))
}

/** 读取背景透明度，并将旧的材质枚举迁移为连续数值。 */
export async function getTaskbarBackgroundTransparency(): Promise<number> {
  const saved = normalizeTaskbarBackgroundTransparency(
    await settingsStore.get<unknown>(TASKBAR_BACKGROUND_TRANSPARENCY_KEY),
  )
  if (saved !== undefined) {
    return saved
  }

  const legacyMaterial = await settingsStore.get<unknown>(LEGACY_TASKBAR_MATERIAL_KEY)
  const migrated =
    legacyMaterial === 'transparent'
      ? TASKBAR_TRANSPARENCY_MAX
      : DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY
  await settingsStore.set(TASKBAR_BACKGROUND_TRANSPARENCY_KEY, migrated)
  await settingsStore.delete(LEGACY_TASKBAR_MATERIAL_KEY)
  return migrated
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

  await settingsStore.set(TASKBAR_BACKGROUND_TRANSPARENCY_KEY, normalized)
  await emit(TASKBAR_BACKGROUND_TRANSPARENCY_CHANGED_EVENT, normalized)
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
