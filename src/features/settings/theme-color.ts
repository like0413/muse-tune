import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

export const TASKBAR_THEME_COLOR_SOURCES = ['cover', 'system', 'custom'] as const
export const TASKBAR_THEME_PRESET_COLORS = [
  '#ef4444',
  '#f97316',
  '#f59e0b',
  '#eab308',
  '#84cc16',
  '#22c55e',
  '#10b981',
  '#14b8a6',
  '#06b6d4',
  '#0ea5e9',
  '#3b82f6',
  '#6366f1',
  '#8b5cf6',
  '#ec4899',
] as const
const TASKBAR_THEME_COLOR_KEY = 'taskbar.themeColor'
const TASKBAR_THEME_COLOR_CHANGED_EVENT = 'settings://taskbar-theme-color-changed'
const HEX_COLOR_PATTERN = /^#[0-9a-f]{6}$/i

export type TaskbarThemeColorSource = (typeof TASKBAR_THEME_COLOR_SOURCES)[number]

export interface TaskbarThemeColor {
  source: TaskbarThemeColorSource
  customColor: string
}

export const DEFAULT_TASKBAR_THEME_COLOR: TaskbarThemeColor = {
  source: 'system',
  customColor: '#1677ff',
}

/** 判断主题色来源是否受支持。 */
export function isTaskbarThemeColorSource(value: unknown): value is TaskbarThemeColorSource {
  return typeof value === 'string' && TASKBAR_THEME_COLOR_SOURCES.some((source) => source === value)
}

/** 将颜色值规范为可安全写入 CSS 的六位十六进制格式。 */
export function normalizeHexColor(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const color = value.trim()
  return HEX_COLOR_PATTERN.test(color) ? color.toLowerCase() : null
}

/** 将外部数据规范为完整的主题色设置。 */
export function normalizeTaskbarThemeColor(value: unknown): TaskbarThemeColor {
  const record =
    typeof value === 'object' && value !== null
      ? (value as Partial<Record<keyof TaskbarThemeColor, unknown>>)
      : {}
  return {
    source: isTaskbarThemeColorSource(record.source)
      ? record.source
      : DEFAULT_TASKBAR_THEME_COLOR.source,
    customColor: normalizeHexColor(record.customColor) ?? DEFAULT_TASKBAR_THEME_COLOR.customColor,
  }
}

/** 读取任务栏主题色设置。 */
export async function getTaskbarThemeColor(): Promise<TaskbarThemeColor> {
  return normalizeTaskbarThemeColor(await settingsStore.get<unknown>(TASKBAR_THEME_COLOR_KEY))
}

/** 持久化主题色设置，并通知全部任务栏窗口。 */
export async function setTaskbarThemeColor(value: TaskbarThemeColor): Promise<void> {
  const normalized = normalizeTaskbarThemeColor(value)
  await settingsStore.set(TASKBAR_THEME_COLOR_KEY, normalized)
  await emit(TASKBAR_THEME_COLOR_CHANGED_EVENT, normalized)
}

/** 监听主题色设置变化。 */
export async function listenTaskbarThemeColorChange(
  handler: (value: TaskbarThemeColor) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_THEME_COLOR_CHANGED_EVENT, ({ payload }) => {
    handler(normalizeTaskbarThemeColor(payload))
  })
}
