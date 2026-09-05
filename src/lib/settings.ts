import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { emitTo, listen } from '@tauri-apps/api/event'
import { LazyStore } from '@tauri-apps/plugin-store'

export const TASKBAR_MATERIALS = ['normal', 'transparent', 'acrylic'] as const
export const TASKBAR_PLACEMENTS = ['auto', 'left', 'right'] as const

export type TaskbarMaterial = (typeof TASKBAR_MATERIALS)[number]
export type TaskbarPlacement = (typeof TASKBAR_PLACEMENTS)[number]

const SETTINGS_STORE_PATH = 'settings.json'
const TASKBAR_MATERIAL_KEY = 'taskbar.material'
const TASKBAR_PLACEMENT_KEY = 'taskbar.placement'
const TASKBAR_MATERIAL_CHANGED_EVENT = 'settings://taskbar-material-changed'
const DEFAULT_TASKBAR_MATERIAL: TaskbarMaterial = 'normal'
const DEFAULT_TASKBAR_PLACEMENT: TaskbarPlacement = 'auto'
const settingsStore = new LazyStore(SETTINGS_STORE_PATH, { autoSave: 100 })

/** 判断持久化值是否为受支持的任务栏窗口材质。 */
export function isTaskbarMaterial(value: unknown): value is TaskbarMaterial {
  return typeof value === 'string' && TASKBAR_MATERIALS.some((material) => material === value)
}

/** 判断持久化值是否为受支持的播放器位置。 */
export function isTaskbarPlacement(value: unknown): value is TaskbarPlacement {
  return typeof value === 'string' && TASKBAR_PLACEMENTS.some((placement) => placement === value)
}

/** 读取任务栏窗口材质，缺失或损坏时回退到正常模式。 */
export async function getTaskbarMaterial(): Promise<TaskbarMaterial> {
  const material = await settingsStore.get<unknown>(TASKBAR_MATERIAL_KEY)
  return isTaskbarMaterial(material) ? material : DEFAULT_TASKBAR_MATERIAL
}

/** 读取播放器位置，缺失或损坏时回退到自动模式。 */
export async function getTaskbarPlacement(): Promise<TaskbarPlacement> {
  const placement = await settingsStore.get<unknown>(TASKBAR_PLACEMENT_KEY)
  return isTaskbarPlacement(placement) ? placement : DEFAULT_TASKBAR_PLACEMENT
}

/** 持久化任务栏窗口材质，并通知已运行的任务栏窗口立即应用。 */
export async function setTaskbarMaterial(material: TaskbarMaterial): Promise<void> {
  await settingsStore.set(TASKBAR_MATERIAL_KEY, material)
  await emitTo('taskbar', TASKBAR_MATERIAL_CHANGED_EVENT, material)
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

/** 监听设置窗口发出的任务栏窗口材质变更。 */
export async function listenTaskbarMaterialChange(
  handler: (material: TaskbarMaterial) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_MATERIAL_CHANGED_EVENT, ({ payload }) => {
    if (isTaskbarMaterial(payload)) {
      handler(payload)
    }
  })
}
