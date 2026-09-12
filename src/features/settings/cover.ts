import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

export const TASKBAR_COVER_SHAPES = ['square', 'rounded', 'circle'] as const
export const TASKBAR_COVER_VISIBILITY_MODES = ['always', 'normal', 'lyrics', 'hidden'] as const
const TASKBAR_COVER_APPEARANCE_KEY = 'taskbar.cover.appearance'
const TASKBAR_COVER_APPEARANCE_CHANGED_EVENT = 'settings://taskbar-cover-appearance-changed'

export type TaskbarCoverShape = (typeof TASKBAR_COVER_SHAPES)[number]
export type TaskbarCoverVisibility = (typeof TASKBAR_COVER_VISIBILITY_MODES)[number]
export type TaskbarCoverDisplayMode = 'normal' | 'lyrics'

export interface TaskbarCoverAppearance {
  visibility: TaskbarCoverVisibility
  shape: TaskbarCoverShape
  rotateWhenPlaying: boolean
  showPlayerSource: boolean
}

export const DEFAULT_TASKBAR_COVER_APPEARANCE: TaskbarCoverAppearance = {
  visibility: 'always',
  shape: 'rounded',
  rotateWhenPlaying: false,
  showPlayerSource: true,
}

/** 判断封面形状是否受支持。 */
export function isTaskbarCoverShape(value: unknown): value is TaskbarCoverShape {
  return typeof value === 'string' && TASKBAR_COVER_SHAPES.some((shape) => shape === value)
}

/** 判断封面显示范围是否受支持。 */
export function isTaskbarCoverVisibility(value: unknown): value is TaskbarCoverVisibility {
  return typeof value === 'string' && TASKBAR_COVER_VISIBILITY_MODES.some((mode) => mode === value)
}

/** 判断指定界面模式是否应显示封面。 */
export function isTaskbarCoverVisibleInMode(
  visibility: TaskbarCoverVisibility,
  mode: TaskbarCoverDisplayMode,
): boolean {
  return visibility === 'always' || visibility === mode
}

/** 切换一个界面模式的封面显隐，并映射回唯一的四状态配置。 */
export function updateTaskbarCoverModeVisibility(
  visibility: TaskbarCoverVisibility,
  mode: TaskbarCoverDisplayMode,
  visible: boolean,
): TaskbarCoverVisibility {
  const normalVisible =
    mode === 'normal' ? visible : isTaskbarCoverVisibleInMode(visibility, 'normal')
  const lyricsVisible =
    mode === 'lyrics' ? visible : isTaskbarCoverVisibleInMode(visibility, 'lyrics')

  if (normalVisible && lyricsVisible) return 'always'
  if (normalVisible) return 'normal'
  if (lyricsVisible) return 'lyrics'
  return 'hidden'
}

/** 将外部值规范为完整封面配置，损坏字段单独回退默认值。 */
export function normalizeTaskbarCoverAppearance(value: unknown): TaskbarCoverAppearance {
  const candidate = typeof value === 'object' && value !== null ? value : {}
  const record = candidate as Partial<Record<keyof TaskbarCoverAppearance | 'visible', unknown>>
  // 旧版只有一个 visible 开关，迁移时保持原来的全局显示或隐藏语义。
  const legacyVisibility =
    typeof record.visible === 'boolean'
      ? record.visible
        ? 'always'
        : 'hidden'
      : DEFAULT_TASKBAR_COVER_APPEARANCE.visibility
  return {
    visibility: isTaskbarCoverVisibility(record.visibility) ? record.visibility : legacyVisibility,
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
