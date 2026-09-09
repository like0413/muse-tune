import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { SETTINGS_SCHEMA_VERSIONS } from './storage/schema-versions'
import { getVersionedSetting, setVersionedSetting } from './storage/versioned-setting'

const TASKBAR_LYRICS_KEY = 'taskbar.lyrics'
const TASKBAR_LYRICS_CHANGED_EVENT = 'settings://taskbar-lyrics-changed'
const TASKBAR_LYRICS_ALIGNMENTS = ['left', 'center', 'right'] as const
const TASKBAR_LYRICS_LINE_MODES = ['single', 'double'] as const

export type TaskbarLyricsAlignment = (typeof TASKBAR_LYRICS_ALIGNMENTS)[number]
export type TaskbarLyricsLineMode = (typeof TASKBAR_LYRICS_LINE_MODES)[number]

export interface TaskbarLyricsSettings {
  enabled: boolean
  alignment: TaskbarLyricsAlignment
  lineMode: TaskbarLyricsLineMode
  wordHighlight: boolean
}

export const DEFAULT_TASKBAR_LYRICS_SETTINGS: TaskbarLyricsSettings = {
  enabled: true,
  alignment: 'left',
  lineMode: 'double',
  wordHighlight: true,
}

/** 判断外部值是否为支持的歌词对齐方式。 */
export function isTaskbarLyricsAlignment(value: unknown): value is TaskbarLyricsAlignment {
  return TASKBAR_LYRICS_ALIGNMENTS.some((alignment) => alignment === value)
}

/** 判断外部值是否为支持的歌词行数模式。 */
export function isTaskbarLyricsLineMode(value: unknown): value is TaskbarLyricsLineMode {
  return TASKBAR_LYRICS_LINE_MODES.some((mode) => mode === value)
}

/** 将持久化或事件数据收敛为完整歌词显示配置。 */
export function normalizeTaskbarLyricsSettings(value: unknown): TaskbarLyricsSettings {
  const candidate = typeof value === 'object' && value !== null ? value : {}
  const record = candidate as Partial<Record<keyof TaskbarLyricsSettings, unknown>>
  return {
    enabled:
      typeof record.enabled === 'boolean'
        ? record.enabled
        : DEFAULT_TASKBAR_LYRICS_SETTINGS.enabled,
    alignment: isTaskbarLyricsAlignment(record.alignment)
      ? record.alignment
      : DEFAULT_TASKBAR_LYRICS_SETTINGS.alignment,
    lineMode: isTaskbarLyricsLineMode(record.lineMode)
      ? record.lineMode
      : DEFAULT_TASKBAR_LYRICS_SETTINGS.lineMode,
    wordHighlight:
      typeof record.wordHighlight === 'boolean'
        ? record.wordHighlight
        : DEFAULT_TASKBAR_LYRICS_SETTINGS.wordHighlight,
  }
}

const lyricsStorage = {
  key: TASKBAR_LYRICS_KEY,
  version: SETTINGS_SCHEMA_VERSIONS.taskbar.lyrics,
  defaultValue: DEFAULT_TASKBAR_LYRICS_SETTINGS,
  normalize: normalizeTaskbarLyricsSettings,
}

/** 读取版本化歌词显示配置。 */
export function getTaskbarLyricsSettings(): Promise<TaskbarLyricsSettings> {
  return getVersionedSetting(lyricsStorage)
}

/** 保存显示配置、同步后端解析开关并广播到全部任务栏窗口。 */
export async function setTaskbarLyricsSettings(value: TaskbarLyricsSettings): Promise<void> {
  const previous = await getTaskbarLyricsSettings()
  const saved = await setVersionedSetting(lyricsStorage, value)
  try {
    await invoke('set_lyrics_enabled', { enabled: saved.enabled })
  } catch (error) {
    await setVersionedSetting(lyricsStorage, previous)
    throw error
  }
  await emit(TASKBAR_LYRICS_CHANGED_EVENT, saved)
}

/** 监听歌词显示配置变化。 */
export function listenTaskbarLyricsSettingsChange(
  handler: (settings: TaskbarLyricsSettings) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_LYRICS_CHANGED_EVENT, ({ payload }) => {
    handler(normalizeTaskbarLyricsSettings(payload))
  })
}
