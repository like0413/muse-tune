import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'
import { clamp } from 'es-toolkit'

import { setLyricsPreferences } from '@/features/lyrics/client'
import { LYRICS_ONLINE_STRATEGIES, type LyricsOnlineStrategy } from '@/features/lyrics/types'

import { SETTINGS_SCHEMA_VERSIONS } from './storage/schema-versions'
import { loadVersionedSetting, setVersionedSetting } from './storage/versioned-setting'
import { normalizeHexColor } from './theme-color'

const TASKBAR_LYRICS_KEY = 'taskbar.lyrics'
const TASKBAR_LYRICS_CHANGED_EVENT = 'settings://taskbar-lyrics-changed'
const TASKBAR_LYRICS_ALIGNMENTS = ['left', 'center', 'right'] as const
const TASKBAR_LYRICS_LINE_MODES = ['single', 'double'] as const
const TASKBAR_LYRICS_SECONDARY_LINES = ['translation_only', 'next', 'translation_or_next'] as const
const TASKBAR_LYRICS_NETWORK_POLICIES = ['auto', 'local_only'] as const
const TASKBAR_LYRICS_ANIMATIONS = ['none', 'up'] as const
const TASKBAR_LYRICS_COLOR_SCHEMES = ['theme', 'custom'] as const
const MAX_FONT_FAMILY_LENGTH = 128
export const TASKBAR_LYRICS_FONT_SIZE_MIN = 10
export const TASKBAR_LYRICS_FONT_SIZE_MAX = 18
export const TASKBAR_LYRICS_TIMING_OFFSET_MIN = -2000
export const TASKBAR_LYRICS_TIMING_OFFSET_MAX = 2000

export type TaskbarLyricsAlignment = (typeof TASKBAR_LYRICS_ALIGNMENTS)[number]
export type TaskbarLyricsLineMode = (typeof TASKBAR_LYRICS_LINE_MODES)[number]
export type TaskbarLyricsSecondaryLine = (typeof TASKBAR_LYRICS_SECONDARY_LINES)[number]
export type TaskbarLyricsNetworkPolicy = (typeof TASKBAR_LYRICS_NETWORK_POLICIES)[number]
export type TaskbarLyricsAnimation = (typeof TASKBAR_LYRICS_ANIMATIONS)[number]
export type TaskbarLyricsColorScheme = (typeof TASKBAR_LYRICS_COLOR_SCHEMES)[number]

export interface TaskbarLyricsSettings {
  enabled: boolean
  alignment: TaskbarLyricsAlignment
  lineMode: TaskbarLyricsLineMode
  secondaryLine: TaskbarLyricsSecondaryLine
  networkPolicy: TaskbarLyricsNetworkPolicy
  onlineStrategy: LyricsOnlineStrategy
  timingOffsetMs: number
  wordHighlight: boolean
  animation: TaskbarLyricsAnimation
  animationPreRoll: boolean
  fontSize: number
  colorScheme: TaskbarLyricsColorScheme
  playedColor: string
  unplayedColor: string
  fontFamily: string
}

export const DEFAULT_TASKBAR_LYRICS_SETTINGS: TaskbarLyricsSettings = {
  enabled: true,
  alignment: 'left',
  lineMode: 'double',
  secondaryLine: 'translation_or_next',
  networkPolicy: 'auto',
  onlineStrategy: 'parallel',
  timingOffsetMs: 0,
  wordHighlight: true,
  animation: 'up',
  animationPreRoll: true,
  fontSize: 14,
  colorScheme: 'theme',
  playedColor: '#1677ff',
  unplayedColor: '#adb1b3',
  fontFamily: '',
}

/** 判断外部值是否为支持的歌词对齐方式。 */
export function isTaskbarLyricsAlignment(value: unknown): value is TaskbarLyricsAlignment {
  return TASKBAR_LYRICS_ALIGNMENTS.some((alignment) => alignment === value)
}

/** 判断外部值是否为支持的歌词行数模式。 */
export function isTaskbarLyricsLineMode(value: unknown): value is TaskbarLyricsLineMode {
  return TASKBAR_LYRICS_LINE_MODES.some((mode) => mode === value)
}

/** 判断外部值是否为支持的双行次要内容。 */
export function isTaskbarLyricsSecondaryLine(value: unknown): value is TaskbarLyricsSecondaryLine {
  return TASKBAR_LYRICS_SECONDARY_LINES.some((secondaryLine) => secondaryLine === value)
}

/** 判断外部值是否为支持的联网策略。 */
export function isTaskbarLyricsNetworkPolicy(value: unknown): value is TaskbarLyricsNetworkPolicy {
  return TASKBAR_LYRICS_NETWORK_POLICIES.some((policy) => policy === value)
}

/** 判断外部值是否为支持的在线歌词调度策略。 */
export function isTaskbarLyricsOnlineStrategy(value: unknown): value is LyricsOnlineStrategy {
  return LYRICS_ONLINE_STRATEGIES.some((strategy) => strategy === value)
}

/** 将时间偏移限制到可校准的范围；正值表示延后显示。 */
export function normalizeTaskbarLyricsTimingOffset(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return DEFAULT_TASKBAR_LYRICS_SETTINGS.timingOffsetMs
  }
  return Math.round(
    clamp(value, TASKBAR_LYRICS_TIMING_OFFSET_MIN, TASKBAR_LYRICS_TIMING_OFFSET_MAX),
  )
}

/** 判断外部值是否为支持的歌词切换动画。 */
export function isTaskbarLyricsAnimation(value: unknown): value is TaskbarLyricsAnimation {
  return TASKBAR_LYRICS_ANIMATIONS.some((animation) => animation === value)
}

/** 判断外部值是否为支持的歌词配色方案。 */
export function isTaskbarLyricsColorScheme(value: unknown): value is TaskbarLyricsColorScheme {
  return TASKBAR_LYRICS_COLOR_SCHEMES.some((scheme) => scheme === value)
}

/** 规范系统字体族名称，空字符串表示继承任务栏原字体。 */
export function normalizeTaskbarLyricsFontFamily(value: unknown): string {
  if (typeof value !== 'string') return DEFAULT_TASKBAR_LYRICS_SETTINGS.fontFamily
  const fontFamily = value.trim()
  const hasControlCharacter = Array.from(fontFamily).some((character) => {
    const codePoint = character.codePointAt(0) ?? 0
    return codePoint < 32 || codePoint === 127
  })
  if (fontFamily.length > MAX_FONT_FAMILY_LENGTH || hasControlCharacter) {
    return DEFAULT_TASKBAR_LYRICS_SETTINGS.fontFamily
  }
  return fontFamily
}

/** 仅保留跟随主题与自定义配色，已移除的旧预设回退为跟随主题。 */
function normalizeTaskbarLyricsColorScheme(value: unknown): TaskbarLyricsColorScheme {
  if (isTaskbarLyricsColorScheme(value)) return value
  return DEFAULT_TASKBAR_LYRICS_SETTINGS.colorScheme
}

/** 将字号限制到任务栏双行布局也不会被上下裁切的范围。 */
export function normalizeTaskbarLyricsFontSize(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return DEFAULT_TASKBAR_LYRICS_SETTINGS.fontSize
  }
  return Math.round(clamp(value, TASKBAR_LYRICS_FONT_SIZE_MIN, TASKBAR_LYRICS_FONT_SIZE_MAX))
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
    secondaryLine:
      record.secondaryLine === 'translation'
        ? 'translation_or_next'
        : isTaskbarLyricsSecondaryLine(record.secondaryLine)
          ? record.secondaryLine
          : DEFAULT_TASKBAR_LYRICS_SETTINGS.secondaryLine,
    networkPolicy: isTaskbarLyricsNetworkPolicy(record.networkPolicy)
      ? record.networkPolicy
      : DEFAULT_TASKBAR_LYRICS_SETTINGS.networkPolicy,
    onlineStrategy: isTaskbarLyricsOnlineStrategy(record.onlineStrategy)
      ? record.onlineStrategy
      : DEFAULT_TASKBAR_LYRICS_SETTINGS.onlineStrategy,
    timingOffsetMs: normalizeTaskbarLyricsTimingOffset(record.timingOffsetMs),
    wordHighlight:
      typeof record.wordHighlight === 'boolean'
        ? record.wordHighlight
        : DEFAULT_TASKBAR_LYRICS_SETTINGS.wordHighlight,
    animation: isTaskbarLyricsAnimation(record.animation)
      ? record.animation
      : DEFAULT_TASKBAR_LYRICS_SETTINGS.animation,
    animationPreRoll:
      typeof record.animationPreRoll === 'boolean'
        ? record.animationPreRoll
        : DEFAULT_TASKBAR_LYRICS_SETTINGS.animationPreRoll,
    fontSize: normalizeTaskbarLyricsFontSize(record.fontSize),
    colorScheme: normalizeTaskbarLyricsColorScheme(record.colorScheme),
    playedColor:
      normalizeHexColor(record.playedColor) ?? DEFAULT_TASKBAR_LYRICS_SETTINGS.playedColor,
    unplayedColor:
      normalizeHexColor(record.unplayedColor) ?? DEFAULT_TASKBAR_LYRICS_SETTINGS.unplayedColor,
    fontFamily: normalizeTaskbarLyricsFontFamily(record.fontFamily),
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
  return loadVersionedSetting(lyricsStorage)
}

/** 保存显示配置、同步后端解析开关并广播到全部任务栏窗口。 */
export async function setTaskbarLyricsSettings(value: TaskbarLyricsSettings): Promise<void> {
  const previous = await getTaskbarLyricsSettings()
  const saved = await setVersionedSetting(lyricsStorage, value)
  if (
    saved.enabled !== previous.enabled ||
    saved.networkPolicy !== previous.networkPolicy ||
    saved.onlineStrategy !== previous.onlineStrategy
  ) {
    try {
      await setLyricsPreferences({
        enabled: saved.enabled,
        allowOnline: saved.networkPolicy === 'auto',
        onlineStrategy: saved.onlineStrategy,
      })
    } catch (error) {
      await setVersionedSetting(lyricsStorage, previous)
      throw error
    }
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
