import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'
import { isEqual, uniq } from 'es-toolkit'

import type { ApplicationLocale } from '@/features/i18n/locales'
import { getApplicationLocale } from '@/features/i18n/settings'
import { setLyricsPreferences } from '@/features/lyrics/client'
import {
  LYRICS_ONLINE_STRATEGIES,
  type LyricsChineseVariant,
  type LyricsOnlineStrategy,
} from '@/features/lyrics/types'
import type { MediaPlayer } from '@/features/media/types'

import { DEFAULT_TASKBAR_LYRICS_SETTINGS, ONLINE_LYRICS_SOURCES } from './defaults'
import { normalizeIntegerInRange } from './normalize'
import { SETTINGS_SCHEMA_VERSIONS } from './storage/schema-versions'
import { loadVersionedSetting, setVersionedSetting } from './storage/versioned-setting'
import { normalizeHexColor } from './theme-color'

const TASKBAR_LYRICS_KEY = 'taskbar.lyrics'
const TASKBAR_LYRICS_CHANGED_EVENT = 'settings://taskbar-lyrics-changed'
const TASKBAR_LYRICS_ALIGNMENTS = ['left', 'center', 'right'] as const
const TASKBAR_LYRICS_LINE_MODES = ['single', 'double'] as const
const TASKBAR_LYRICS_SECONDARY_LINES = ['translation_only', 'next', 'translation_or_next'] as const
const TASKBAR_LYRICS_NETWORK_POLICIES = ['auto', 'local_only'] as const
const TASKBAR_LYRICS_CHINESE_VARIANTS = ['follow_interface', 'simplified', 'traditional'] as const
const TASKBAR_LYRICS_ANIMATIONS = ['none', 'up', 'fade'] as const
const TASKBAR_LYRICS_COLOR_SCHEMES = ['theme', 'custom'] as const
const MAX_FONT_FAMILY_LENGTH = 128
export const TASKBAR_LYRICS_FONT_SIZE_MIN = 10
export const TASKBAR_LYRICS_FONT_SIZE_MAX = 18
/** 时间偏移的可调范围；设置页滑杆按此区间与 50ms 步进取值，任务栏据此换算歌词位置。 */
export const TASKBAR_LYRICS_TIMING_OFFSET_MIN = -2000
export const TASKBAR_LYRICS_TIMING_OFFSET_MAX = 2000

export type TaskbarLyricsAlignment = (typeof TASKBAR_LYRICS_ALIGNMENTS)[number]
export type TaskbarLyricsLineMode = (typeof TASKBAR_LYRICS_LINE_MODES)[number]
export type TaskbarLyricsSecondaryLine = (typeof TASKBAR_LYRICS_SECONDARY_LINES)[number]
export type TaskbarLyricsNetworkPolicy = (typeof TASKBAR_LYRICS_NETWORK_POLICIES)[number]
export type TaskbarLyricsChineseVariant = (typeof TASKBAR_LYRICS_CHINESE_VARIANTS)[number]
export type TaskbarLyricsAnimation = (typeof TASKBAR_LYRICS_ANIMATIONS)[number]
export type TaskbarLyricsColorScheme = (typeof TASKBAR_LYRICS_COLOR_SCHEMES)[number]

export interface TaskbarLyricsSettings {
  enabled: boolean
  chineseVariant: TaskbarLyricsChineseVariant
  alignment: TaskbarLyricsAlignment
  lineMode: TaskbarLyricsLineMode
  secondaryLine: TaskbarLyricsSecondaryLine
  networkPolicy: TaskbarLyricsNetworkPolicy
  onlineStrategy: LyricsOnlineStrategy
  /** 全部在线歌词接口的顺序；始终是每个平台各出现一次的完整排列。 */
  onlineSourceOrder: MediaPlayer[]
  /** 参与在线检索的平台子集，可为空表示不使用任何在线接口。 */
  enabledOnlineSources: MediaPlayer[]
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

export { DEFAULT_TASKBAR_LYRICS_SETTINGS }

export function isTaskbarLyricsAlignment(value: unknown): value is TaskbarLyricsAlignment {
  return TASKBAR_LYRICS_ALIGNMENTS.some((alignment) => alignment === value)
}

export function isTaskbarLyricsLineMode(value: unknown): value is TaskbarLyricsLineMode {
  return TASKBAR_LYRICS_LINE_MODES.some((mode) => mode === value)
}

export function isTaskbarLyricsSecondaryLine(value: unknown): value is TaskbarLyricsSecondaryLine {
  return TASKBAR_LYRICS_SECONDARY_LINES.some((secondaryLine) => secondaryLine === value)
}

export function isTaskbarLyricsNetworkPolicy(value: unknown): value is TaskbarLyricsNetworkPolicy {
  return TASKBAR_LYRICS_NETWORK_POLICIES.some((policy) => policy === value)
}

export function isTaskbarLyricsChineseVariant(
  value: unknown,
): value is TaskbarLyricsChineseVariant {
  return TASKBAR_LYRICS_CHINESE_VARIANTS.some((variant) => variant === value)
}

export function isTaskbarLyricsOnlineStrategy(value: unknown): value is LyricsOnlineStrategy {
  return LYRICS_ONLINE_STRATEGIES.some((strategy) => strategy === value)
}

/** 将时间偏移限制到可校准的范围；正值表示延后显示。 */
export function normalizeTaskbarLyricsTimingOffset(value: unknown): number {
  return (
    normalizeIntegerInRange(
      value,
      TASKBAR_LYRICS_TIMING_OFFSET_MIN,
      TASKBAR_LYRICS_TIMING_OFFSET_MAX,
    ) ?? DEFAULT_TASKBAR_LYRICS_SETTINGS.timingOffsetMs
  )
}

export function isTaskbarLyricsAnimation(value: unknown): value is TaskbarLyricsAnimation {
  return TASKBAR_LYRICS_ANIMATIONS.some((animation) => animation === value)
}

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
  return (
    normalizeIntegerInRange(value, TASKBAR_LYRICS_FONT_SIZE_MIN, TASKBAR_LYRICS_FONT_SIZE_MAX) ??
    DEFAULT_TASKBAR_LYRICS_SETTINGS.fontSize
  )
}

/** 从外部值中筛出受支持的已接入平台，并按首次出现顺序去重。 */
function filterSupportedMediaPlayers(value: unknown): MediaPlayer[] {
  if (!Array.isArray(value)) return []
  return uniq(
    value.filter((player): player is MediaPlayer =>
      ONLINE_LYRICS_SOURCES.some((supported) => supported === player),
    ),
  )
}

/** 规范化在线接口顺序：缺失或损坏时回退默认，否则按规范平台顺序补齐为完整排列。 */
function normalizeTaskbarLyricsOnlineSourceOrder(value: unknown): MediaPlayer[] {
  if (!Array.isArray(value)) return [...DEFAULT_TASKBAR_LYRICS_SETTINGS.onlineSourceOrder]
  return uniq([...filterSupportedMediaPlayers(value), ...ONLINE_LYRICS_SOURCES])
}

/**
 * 规范化参与在线检索的平台：缺失或损坏时回退默认启用集合，允许空数组表示完全停用在线接口。
 *
 * 只保留出现在归一化顺序里的平台，确保启用集合始终是顺序集合的子集。
 */
function normalizeTaskbarLyricsEnabledOnlineSources(
  value: unknown,
  onlineSourceOrder: readonly MediaPlayer[],
): MediaPlayer[] {
  if (!Array.isArray(value)) return [...DEFAULT_TASKBAR_LYRICS_SETTINGS.enabledOnlineSources]
  const available = new Set(onlineSourceOrder)
  return filterSupportedMediaPlayers(value).filter((player) => available.has(player))
}

/** 将持久化或事件数据收敛为完整歌词显示配置。 */
export function normalizeTaskbarLyricsSettings(value: unknown): TaskbarLyricsSettings {
  const candidate = typeof value === 'object' && value !== null ? value : {}
  const record = candidate as Partial<Record<keyof TaskbarLyricsSettings, unknown>>
  // 启用集合的合法性依赖归一化后的顺序，因此先算出完整排列再收敛子集。
  const onlineSourceOrder = normalizeTaskbarLyricsOnlineSourceOrder(record.onlineSourceOrder)
  return {
    enabled:
      typeof record.enabled === 'boolean'
        ? record.enabled
        : DEFAULT_TASKBAR_LYRICS_SETTINGS.enabled,
    chineseVariant: isTaskbarLyricsChineseVariant(record.chineseVariant)
      ? record.chineseVariant
      : DEFAULT_TASKBAR_LYRICS_SETTINGS.chineseVariant,
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
    onlineSourceOrder,
    enabledOnlineSources: normalizeTaskbarLyricsEnabledOnlineSources(
      record.enabledOnlineSources,
      onlineSourceOrder,
    ),
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

/**
 * 计算生效的在线接口：按偏好顺序取与启用集合的交集。
 *
 * 这个集合只服务于并行策略：当前播放平台不在这里补（原生侧会隐式补在最前，未勾选也会查询），
 * 仅当前平台策略则完全忽略它。
 */
function resolveOnlineSources(settings: TaskbarLyricsSettings): MediaPlayer[] {
  const enabled = new Set(settings.enabledOnlineSources)
  return settings.onlineSourceOrder.filter((player) => enabled.has(player))
}

/** 将歌词显示配置映射为后端歌词解析偏好；原生侧值未变化时是空操作。 */
async function applyLyricsPreferences(
  settings: TaskbarLyricsSettings,
  locale?: ApplicationLocale,
): Promise<void> {
  const effectiveLocale = locale ?? (await getApplicationLocale())
  await setLyricsPreferences({
    enabled: settings.enabled,
    chineseVariant: resolveLyricsChineseVariant(settings.chineseVariant, effectiveLocale),
    allowOnline: settings.networkPolicy === 'auto',
    onlineStrategy: settings.onlineStrategy,
    onlineSources: resolveOnlineSources(settings),
  })
}

/** 把持久化选项解析成后端实际执行的转换目标。 */
export function resolveLyricsChineseVariant(
  preference: TaskbarLyricsChineseVariant,
  locale: ApplicationLocale,
): LyricsChineseVariant {
  if (preference === 'simplified' || preference === 'traditional') return preference
  if (locale === 'zh-Hans') return 'simplified'
  if (locale === 'zh-Hant') return 'traditional'
  return 'original'
}

/** 界面语言变化时，仅刷新“跟随界面”的实际输出目标，不改写歌词设置。 */
export async function syncTaskbarLyricsChineseVariant(locale: ApplicationLocale): Promise<void> {
  const settings = await getTaskbarLyricsSettings()
  if (settings.chineseVariant === 'follow_interface') {
    await applyLyricsPreferences(settings, locale)
  }
}

/**
 * 保存显示配置、同步后端解析开关并广播到全部任务栏窗口。
 *
 * 必须经由本函数写入：`enabled` / `networkPolicy` / `onlineStrategy` 与在线接口顺序、
 * 启用集合只在这里推送给后端（写入契约见 `defaults.ts`）。
 */
export async function setTaskbarLyricsSettings(value: TaskbarLyricsSettings): Promise<void> {
  const previous = await getTaskbarLyricsSettings()
  const saved = await setVersionedSetting(lyricsStorage, value)
  if (
    saved.enabled !== previous.enabled ||
    saved.chineseVariant !== previous.chineseVariant ||
    saved.networkPolicy !== previous.networkPolicy ||
    saved.onlineStrategy !== previous.onlineStrategy ||
    !isEqual(saved.enabledOnlineSources, previous.enabledOnlineSources) ||
    !isEqual(saved.onlineSourceOrder, previous.onlineSourceOrder)
  ) {
    try {
      await applyLyricsPreferences(saved)
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
