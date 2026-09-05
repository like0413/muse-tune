import type { UnlistenFn } from '@tauri-apps/api/event'
import { emitTo, listen } from '@tauri-apps/api/event'
import { LazyStore } from '@tauri-apps/plugin-store'

export const TASKBAR_MATERIALS = ['normal', 'transparent', 'acrylic'] as const

export type TaskbarMaterial = (typeof TASKBAR_MATERIALS)[number]

const SETTINGS_STORE_PATH = 'settings.json'
const TASKBAR_MATERIAL_KEY = 'taskbar.material'
const TASKBAR_MATERIAL_CHANGED_EVENT = 'settings://taskbar-material-changed'
const DEFAULT_TASKBAR_MATERIAL: TaskbarMaterial = 'normal'
const settingsStore = new LazyStore(SETTINGS_STORE_PATH, { autoSave: 100 })

/** 判断持久化值是否为受支持的任务栏窗口材质。 */
export function isTaskbarMaterial(value: unknown): value is TaskbarMaterial {
  return typeof value === 'string' && TASKBAR_MATERIALS.some((material) => material === value)
}

/** 读取任务栏窗口材质，缺失或损坏时回退到正常模式。 */
export async function getTaskbarMaterial(): Promise<TaskbarMaterial> {
  const material = await settingsStore.get<unknown>(TASKBAR_MATERIAL_KEY)
  return isTaskbarMaterial(material) ? material : DEFAULT_TASKBAR_MATERIAL
}

/** 持久化任务栏窗口材质，并通知已运行的任务栏窗口立即应用。 */
export async function setTaskbarMaterial(material: TaskbarMaterial): Promise<void> {
  await settingsStore.set(TASKBAR_MATERIAL_KEY, material)
  await emitTo('taskbar', TASKBAR_MATERIAL_CHANGED_EVENT, material)
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
