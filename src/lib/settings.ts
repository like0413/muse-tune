import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { emitTo, listen } from '@tauri-apps/api/event'
import { LazyStore } from '@tauri-apps/plugin-store'
import { clamp } from 'es-toolkit'

export const TASKBAR_PLACEMENTS = ['auto', 'left', 'right'] as const
export const TASKBAR_OVERLAP_PRIORITIES = ['bar', 'taskbar'] as const
export const TASKBAR_TRANSPARENCY_MIN = 0
export const TASKBAR_TRANSPARENCY_MAX = 100

export type TaskbarPlacement = (typeof TASKBAR_PLACEMENTS)[number]
export type TaskbarOverlapPriority = (typeof TASKBAR_OVERLAP_PRIORITIES)[number]

const SETTINGS_STORE_PATH = 'settings.json'
const LEGACY_TASKBAR_MATERIAL_KEY = 'taskbar.material'
const TASKBAR_BACKGROUND_TRANSPARENCY_KEY = 'taskbar.backgroundTransparency'
const TASKBAR_PLACEMENT_KEY = 'taskbar.placement'
const TASKBAR_OVERLAP_PRIORITY_KEY = 'taskbar.overlapPriority'
const TASKBAR_BACKGROUND_TRANSPARENCY_CHANGED_EVENT =
  'settings://taskbar-background-transparency-changed'
const DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY = 0
const DEFAULT_TASKBAR_PLACEMENT: TaskbarPlacement = 'auto'
const DEFAULT_TASKBAR_OVERLAP_PRIORITY: TaskbarOverlapPriority = 'bar'
const settingsStore = new LazyStore(SETTINGS_STORE_PATH, { autoSave: 100 })

/** 将外部透明度值规范到 0～100 的整数范围。 */
export function normalizeTaskbarBackgroundTransparency(value: unknown): number | undefined {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return undefined
  }

  return Math.round(clamp(value, TASKBAR_TRANSPARENCY_MIN, TASKBAR_TRANSPARENCY_MAX))
}

/** 判断持久化值是否为受支持的播放器位置。 */
export function isTaskbarPlacement(value: unknown): value is TaskbarPlacement {
  return typeof value === 'string' && TASKBAR_PLACEMENTS.some((placement) => placement === value)
}

/** 判断持久化值是否为受支持的任务栏元素遮挡优先级。 */
export function isTaskbarOverlapPriority(value: unknown): value is TaskbarOverlapPriority {
  return (
    typeof value === 'string' && TASKBAR_OVERLAP_PRIORITIES.some((priority) => priority === value)
  )
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

/** 读取播放器位置，缺失或损坏时回退到自动模式。 */
export async function getTaskbarPlacement(): Promise<TaskbarPlacement> {
  const placement = await settingsStore.get<unknown>(TASKBAR_PLACEMENT_KEY)
  return isTaskbarPlacement(placement) ? placement : DEFAULT_TASKBAR_PLACEMENT
}

/** 读取任务栏元素遮挡优先级，缺失或损坏时维持播放器优先。 */
export async function getTaskbarOverlapPriority(): Promise<TaskbarOverlapPriority> {
  const priority = await settingsStore.get<unknown>(TASKBAR_OVERLAP_PRIORITY_KEY)
  return isTaskbarOverlapPriority(priority) ? priority : DEFAULT_TASKBAR_OVERLAP_PRIORITY
}

/** 仅预览背景透明度，不触发持久化写入。 */
export async function previewTaskbarBackgroundTransparency(transparency: number): Promise<void> {
  const normalized = normalizeTaskbarBackgroundTransparency(transparency)
  if (normalized !== undefined) {
    await emitTo('taskbar', TASKBAR_BACKGROUND_TRANSPARENCY_CHANGED_EVENT, normalized)
  }
}

/** 持久化背景透明度，并通知已运行的任务栏窗口应用最终值。 */
export async function setTaskbarBackgroundTransparency(transparency: number): Promise<void> {
  const normalized = normalizeTaskbarBackgroundTransparency(transparency)
  if (normalized === undefined) {
    return
  }

  await settingsStore.set(TASKBAR_BACKGROUND_TRANSPARENCY_KEY, normalized)
  await emitTo('taskbar', TASKBAR_BACKGROUND_TRANSPARENCY_CHANGED_EVENT, normalized)
}

/** 立即应用播放器位置，并在成功后持久化选择。 */
export async function setTaskbarPlacement(placement: TaskbarPlacement): Promise<void> {
  await applyTaskbarPlacement(placement)
  await settingsStore.set(TASKBAR_PLACEMENT_KEY, placement)
}

/** 将播放器位置同步到原生任务栏定位线程。 */
export async function applyTaskbarPlacement(placement: TaskbarPlacement): Promise<void> {
  await invoke('set_taskbar_placement', { placement })
}

/** 立即应用任务栏元素遮挡优先级，并在成功后持久化选择。 */
export async function setTaskbarOverlapPriority(priority: TaskbarOverlapPriority): Promise<void> {
  await applyTaskbarOverlapPriority(priority)
  await settingsStore.set(TASKBAR_OVERLAP_PRIORITY_KEY, priority)
}

/** 将任务栏元素遮挡优先级同步到原生任务栏定位线程。 */
export async function applyTaskbarOverlapPriority(priority: TaskbarOverlapPriority): Promise<void> {
  await invoke('set_taskbar_overlap_priority', { priority })
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
