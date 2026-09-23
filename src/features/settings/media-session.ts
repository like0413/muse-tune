import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'
import { uniq } from 'es-toolkit'

import type {
  MediaPlayer,
  MediaSessionSelectionPolicy,
  MediaSessionSelectionStrategy,
} from '@/features/media/types'

import {
  DEFAULT_MEDIA_SESSION_ONLY_SUPPORTED_PLAYERS,
  DEFAULT_MEDIA_SESSION_SELECTION_STRATEGY,
  SUPPORTED_MEDIA_PLAYERS,
} from './defaults'
import { settingsStore } from './store'

const MEDIA_SESSION_SELECTION_STRATEGIES = [
  'follow_windows',
  'recent_playback',
  'sticky_current',
  'fixed_priority',
] as const satisfies readonly MediaSessionSelectionStrategy[]

const MEDIA_SESSION_SELECTION_KEY = 'media.sessionSelection'
const MEDIA_SESSION_SELECTION_CHANGED_EVENT = 'settings://media-session-selection-changed'

/** 会话选择策略默认值；`playerPriority` 取共享配置里的规范平台顺序，其余同样来自共享配置。 */
export const DEFAULT_MEDIA_SESSION_SELECTION_POLICY: MediaSessionSelectionPolicy = {
  strategy: DEFAULT_MEDIA_SESSION_SELECTION_STRATEGY,
  playerPriority: [...SUPPORTED_MEDIA_PLAYERS],
  onlySupportedPlayers: DEFAULT_MEDIA_SESSION_ONLY_SUPPORTED_PLAYERS,
}

export function isMediaSessionSelectionStrategy(
  value: unknown,
): value is MediaSessionSelectionStrategy {
  return (
    typeof value === 'string' &&
    MEDIA_SESSION_SELECTION_STRATEGIES.some((strategy) => strategy === value)
  )
}

/** 将损坏或旧版配置规范为完整策略，并补齐全部已接入播放器。 */
export function normalizeMediaSessionSelectionPolicy(value: unknown): MediaSessionSelectionPolicy {
  const candidate = typeof value === 'object' && value !== null ? value : {}
  const record = candidate as Partial<Record<keyof MediaSessionSelectionPolicy, unknown>>
  const inputPriority = Array.isArray(record.playerPriority) ? record.playerPriority : []
  // 先按持久化顺序去重保序，再补齐缺失的播放器，保证优先级列表完整且不重复。
  const playerPriority = uniq([
    ...inputPriority.filter((player): player is MediaPlayer =>
      SUPPORTED_MEDIA_PLAYERS.some((supported) => supported === player),
    ),
    ...SUPPORTED_MEDIA_PLAYERS,
  ])

  return {
    strategy: isMediaSessionSelectionStrategy(record.strategy)
      ? record.strategy
      : DEFAULT_MEDIA_SESSION_SELECTION_POLICY.strategy,
    playerPriority,
    onlySupportedPlayers:
      typeof record.onlySupportedPlayers === 'boolean'
        ? record.onlySupportedPlayers
        : DEFAULT_MEDIA_SESSION_SELECTION_POLICY.onlySupportedPlayers,
  }
}

export async function getMediaSessionSelectionPolicy(): Promise<MediaSessionSelectionPolicy> {
  return normalizeMediaSessionSelectionPolicy(
    await settingsStore.get<unknown>(MEDIA_SESSION_SELECTION_KEY),
  )
}

/**
 * 保存选择策略并通知任务栏实时应用。
 *
 * 必须经由本函数写入：原生侧依赖这里广播的变更事件完成推送（写入契约见 `defaults.ts`）。
 */
export async function setMediaSessionSelectionPolicy(
  policy: MediaSessionSelectionPolicy,
): Promise<void> {
  const normalized = normalizeMediaSessionSelectionPolicy(policy)
  await settingsStore.set(MEDIA_SESSION_SELECTION_KEY, normalized)
  await emit(MEDIA_SESSION_SELECTION_CHANGED_EVENT, normalized)
}

export async function listenMediaSessionSelectionPolicyChange(
  handler: (policy: MediaSessionSelectionPolicy) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(MEDIA_SESSION_SELECTION_CHANGED_EVENT, ({ payload }) => {
    handler(normalizeMediaSessionSelectionPolicy(payload))
  })
}
