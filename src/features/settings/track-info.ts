import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import {
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
  DEFAULT_TASKBAR_TRACK_INFO_SCROLLING,
  DEFAULT_TASKBAR_TRACK_INFO_VISIBLE,
} from './defaults'
import { normalizeIntegerInRange } from './normalize'
import { settingsStore } from './store'

const TASKBAR_TRACK_INFO_ALIGNMENTS = ['left', 'right'] as const
export const TASKBAR_TRACK_INFO_SCROLL_MODES = ['loop', 'restart', 'alternate'] as const
export const TASKBAR_TRACK_INFO_SCROLL_SPEED_MIN = 10
export const TASKBAR_TRACK_INFO_SCROLL_SPEED_MAX = 100
const TASKBAR_TRACK_INFO_ALIGNMENT_KEY = 'taskbar.trackInfo.alignment'
const TASKBAR_TRACK_INFO_ALIGNMENT_CHANGED_EVENT = 'settings://taskbar-track-info-alignment-changed'
const TASKBAR_TRACK_INFO_SCROLLING_KEY = 'taskbar.trackInfo.scrolling'
const TASKBAR_TRACK_INFO_SCROLLING_CHANGED_EVENT = 'settings://taskbar-track-info-scrolling-changed'
const TASKBAR_TRACK_INFO_VISIBLE_KEY = 'taskbar.trackInfo.visible'
const TASKBAR_TRACK_INFO_VISIBLE_CHANGED_EVENT = 'settings://taskbar-track-info-visible-changed'

export type TaskbarTrackInfoAlignment = (typeof TASKBAR_TRACK_INFO_ALIGNMENTS)[number]
export type TaskbarTrackInfoScrollMode = (typeof TASKBAR_TRACK_INFO_SCROLL_MODES)[number]

export interface TaskbarTrackInfoScrolling {
  enabled: boolean
  speed: number
  mode: TaskbarTrackInfoScrollMode
}

export {
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
  DEFAULT_TASKBAR_TRACK_INFO_SCROLLING,
  DEFAULT_TASKBAR_TRACK_INFO_VISIBLE,
}

export async function getTaskbarTrackInfoVisible(): Promise<boolean> {
  const value = await settingsStore.get<unknown>(TASKBAR_TRACK_INFO_VISIBLE_KEY)
  return typeof value === 'boolean' ? value : DEFAULT_TASKBAR_TRACK_INFO_VISIBLE
}

export async function setTaskbarTrackInfoVisible(visible: boolean): Promise<void> {
  await settingsStore.set(TASKBAR_TRACK_INFO_VISIBLE_KEY, visible)
  await emit(TASKBAR_TRACK_INFO_VISIBLE_CHANGED_EVENT, visible)
}

export async function listenTaskbarTrackInfoVisibleChange(
  handler: (visible: boolean) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_TRACK_INFO_VISIBLE_CHANGED_EVENT, ({ payload }) => {
    if (typeof payload === 'boolean') handler(payload)
  })
}

/** 判断外部值是否为受支持的歌曲信息对齐方式。 */
export function isTaskbarTrackInfoAlignment(value: unknown): value is TaskbarTrackInfoAlignment {
  return (
    typeof value === 'string' &&
    TASKBAR_TRACK_INFO_ALIGNMENTS.some((alignment) => alignment === value)
  )
}

/** 判断外部值是否为受支持的歌名滚动方式。 */
export function isTaskbarTrackInfoScrollMode(value: unknown): value is TaskbarTrackInfoScrollMode {
  return typeof value === 'string' && TASKBAR_TRACK_INFO_SCROLL_MODES.some((mode) => mode === value)
}

/** 将外部速度规范到设置面板支持的整数像素每秒范围。 */
export function normalizeTaskbarTrackInfoScrollSpeed(value: unknown): number | undefined {
  return normalizeIntegerInRange(
    value,
    TASKBAR_TRACK_INFO_SCROLL_SPEED_MIN,
    TASKBAR_TRACK_INFO_SCROLL_SPEED_MAX,
  )
}

/** 将外部值规范为完整歌名滚动配置，损坏字段独立回退默认值。 */
export function normalizeTaskbarTrackInfoScrolling(value: unknown): TaskbarTrackInfoScrolling {
  const candidate = typeof value === 'object' && value !== null ? value : {}
  const record = candidate as Partial<Record<keyof TaskbarTrackInfoScrolling, unknown>>

  return {
    enabled:
      typeof record.enabled === 'boolean'
        ? record.enabled
        : DEFAULT_TASKBAR_TRACK_INFO_SCROLLING.enabled,
    speed:
      normalizeTaskbarTrackInfoScrollSpeed(record.speed) ??
      DEFAULT_TASKBAR_TRACK_INFO_SCROLLING.speed,
    mode: isTaskbarTrackInfoScrollMode(record.mode)
      ? record.mode
      : DEFAULT_TASKBAR_TRACK_INFO_SCROLLING.mode,
  }
}

export async function getTaskbarTrackInfoAlignment(): Promise<TaskbarTrackInfoAlignment> {
  const alignment = await settingsStore.get<unknown>(TASKBAR_TRACK_INFO_ALIGNMENT_KEY)
  return isTaskbarTrackInfoAlignment(alignment) ? alignment : DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT
}

export async function setTaskbarTrackInfoAlignment(
  alignment: TaskbarTrackInfoAlignment,
): Promise<void> {
  if (!isTaskbarTrackInfoAlignment(alignment)) throw new Error('无效的歌曲信息对齐方式')

  await settingsStore.set(TASKBAR_TRACK_INFO_ALIGNMENT_KEY, alignment)
  await emit(TASKBAR_TRACK_INFO_ALIGNMENT_CHANGED_EVENT, alignment)
}

export async function listenTaskbarTrackInfoAlignmentChange(
  handler: (alignment: TaskbarTrackInfoAlignment) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_TRACK_INFO_ALIGNMENT_CHANGED_EVENT, ({ payload }) => {
    if (isTaskbarTrackInfoAlignment(payload)) handler(payload)
  })
}

export async function getTaskbarTrackInfoScrolling(): Promise<TaskbarTrackInfoScrolling> {
  return normalizeTaskbarTrackInfoScrolling(
    await settingsStore.get<unknown>(TASKBAR_TRACK_INFO_SCROLLING_KEY),
  )
}

/** 只通知任务栏窗口预览歌名滚动配置，不写入存储。 */
export async function applyTaskbarTrackInfoScrolling(
  scrolling: TaskbarTrackInfoScrolling,
): Promise<void> {
  await emit(
    TASKBAR_TRACK_INFO_SCROLLING_CHANGED_EVENT,
    normalizeTaskbarTrackInfoScrolling(scrolling),
  )
}

export async function setTaskbarTrackInfoScrolling(
  scrolling: TaskbarTrackInfoScrolling,
): Promise<void> {
  const normalized = normalizeTaskbarTrackInfoScrolling(scrolling)
  await settingsStore.set(TASKBAR_TRACK_INFO_SCROLLING_KEY, normalized)
  await applyTaskbarTrackInfoScrolling(normalized)
}

export async function listenTaskbarTrackInfoScrollingChange(
  handler: (scrolling: TaskbarTrackInfoScrolling) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_TRACK_INFO_SCROLLING_CHANGED_EVENT, ({ payload }) => {
    handler(normalizeTaskbarTrackInfoScrolling(payload))
  })
}
