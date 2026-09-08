import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import type {
  MediaPlayer,
  MediaSessionSelectionPolicy,
  MediaSessionSelectionStrategy,
} from '@/features/media/types'

import { settingsStore } from './store'

export const MEDIA_SESSION_SELECTION_STRATEGIES = [
  'follow_windows',
  'recent_playback',
  'sticky_current',
  'fixed_priority',
] as const satisfies readonly MediaSessionSelectionStrategy[]
export const SUPPORTED_MEDIA_PLAYERS = [
  'qq_music',
  'netease_cloud_music',
  'soda_music',
  'kugou_music',
] as const satisfies readonly MediaPlayer[]

const MEDIA_SESSION_SELECTION_KEY = 'media.sessionSelection'
const MEDIA_SESSION_SELECTION_CHANGED_EVENT = 'settings://media-session-selection-changed'

export const DEFAULT_MEDIA_SESSION_SELECTION_POLICY: MediaSessionSelectionPolicy = {
  strategy: 'recent_playback',
  playerPriority: [...SUPPORTED_MEDIA_PLAYERS],
  onlySupportedPlayers: false,
}

/** 判断外部值是否为有效会话选择策略。 */
export function isMediaSessionSelectionStrategy(
  value: unknown,
): value is MediaSessionSelectionStrategy {
  return (
    typeof value === 'string' &&
    MEDIA_SESSION_SELECTION_STRATEGIES.some((strategy) => strategy === value)
  )
}

/** 将损坏或旧版配置规范为完整策略，并补齐四家播放器。 */
export function normalizeMediaSessionSelectionPolicy(value: unknown): MediaSessionSelectionPolicy {
  const candidate = typeof value === 'object' && value !== null ? value : {}
  const record = candidate as Partial<Record<keyof MediaSessionSelectionPolicy, unknown>>
  const inputPriority = Array.isArray(record.playerPriority) ? record.playerPriority : []
  const playerPriority = inputPriority.filter(
    (player, index): player is MediaPlayer =>
      SUPPORTED_MEDIA_PLAYERS.some((supported) => supported === player) &&
      inputPriority.indexOf(player) === index,
  )

  for (const player of SUPPORTED_MEDIA_PLAYERS) {
    if (!playerPriority.includes(player)) playerPriority.push(player)
  }

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

/** 读取持久化的多播放器选择策略。 */
export async function getMediaSessionSelectionPolicy(): Promise<MediaSessionSelectionPolicy> {
  return normalizeMediaSessionSelectionPolicy(
    await settingsStore.get<unknown>(MEDIA_SESSION_SELECTION_KEY),
  )
}

/** 保存选择策略并通知任务栏实时应用。 */
export async function setMediaSessionSelectionPolicy(
  policy: MediaSessionSelectionPolicy,
): Promise<void> {
  const normalized = normalizeMediaSessionSelectionPolicy(policy)
  await settingsStore.set(MEDIA_SESSION_SELECTION_KEY, normalized)
  await emit(MEDIA_SESSION_SELECTION_CHANGED_EVENT, normalized)
}

/** 监听多播放器选择策略变化。 */
export async function listenMediaSessionSelectionPolicyChange(
  handler: (policy: MediaSessionSelectionPolicy) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(MEDIA_SESSION_SELECTION_CHANGED_EVENT, ({ payload }) => {
    handler(normalizeMediaSessionSelectionPolicy(payload))
  })
}
