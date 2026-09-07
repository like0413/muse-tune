import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

export const TASKBAR_COVER_SHAPES = ['square', 'rounded', 'circle'] as const
const TASKBAR_COVER_APPEARANCE_KEY = 'taskbar.cover.appearance'
const TASKBAR_COVER_APPEARANCE_CHANGED_EVENT = 'settings://taskbar-cover-appearance-changed'

export type TaskbarCoverShape = (typeof TASKBAR_COVER_SHAPES)[number]

export interface TaskbarCoverAppearance {
  visible: boolean
  shape: TaskbarCoverShape
  rotateWhenPlaying: boolean
  showPlayerSource: boolean
}

export const DEFAULT_TASKBAR_COVER_APPEARANCE: TaskbarCoverAppearance = {
  visible: true,
  shape: 'rounded',
  rotateWhenPlaying: false,
  showPlayerSource: true,
}

/** 判断封面形状是否受支持。 */
export function isTaskbarCoverShape(value: unknown): value is TaskbarCoverShape {
  return typeof value === 'string' && TASKBAR_COVER_SHAPES.some((shape) => shape === value)
}

/** 将外部值规范为完整封面配置，损坏字段单独回退默认值。 */
export function normalizeTaskbarCoverAppearance(value: unknown): TaskbarCoverAppearance {
  const candidate = typeof value === 'object' && value !== null ? value : {}
  const record = candidate as Partial<Record<keyof TaskbarCoverAppearance, unknown>>
  return {
    visible:
      typeof record.visible === 'boolean'
        ? record.visible
        : DEFAULT_TASKBAR_COVER_APPEARANCE.visible,
    shape: isTaskbarCoverShape(record.shape)
      ? record.shape
      : DEFAULT_TASKBAR_COVER_APPEARANCE.shape,
    rotateWhenPlaying:
      typeof record.rotateWhenPlaying === 'boolean'
        ? record.rotateWhenPlaying
        : DEFAULT_TASKBAR_COVER_APPEARANCE.rotateWhenPlaying,
    showPlayerSource:
      typeof record.showPlayerSource === 'boolean'
        ? record.showPlayerSource
        : DEFAULT_TASKBAR_COVER_APPEARANCE.showPlayerSource,
  }
}

/** 读取封面显示配置。 */
export async function getTaskbarCoverAppearance(): Promise<TaskbarCoverAppearance> {
  return normalizeTaskbarCoverAppearance(
    await settingsStore.get<unknown>(TASKBAR_COVER_APPEARANCE_KEY),
  )
}

/** 持久化完整封面配置，并通知全部任务栏窗口。 */
export async function setTaskbarCoverAppearance(appearance: TaskbarCoverAppearance): Promise<void> {
  const normalized = normalizeTaskbarCoverAppearance(appearance)
  await settingsStore.set(TASKBAR_COVER_APPEARANCE_KEY, normalized)
  await emit(TASKBAR_COVER_APPEARANCE_CHANGED_EVENT, normalized)
}

/** 监听封面显示配置变化。 */
export async function listenTaskbarCoverAppearanceChange(
  handler: (appearance: TaskbarCoverAppearance) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_COVER_APPEARANCE_CHANGED_EVENT, ({ payload }) => {
    handler(normalizeTaskbarCoverAppearance(payload))
  })
}
