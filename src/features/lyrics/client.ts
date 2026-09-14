import { invoke } from '@tauri-apps/api/core'

import type { LyricsOnlineStrategy, LyricsSnapshot } from './types'

interface LyricsPreferences {
  enabled: boolean
  allowOnline: boolean
  onlineStrategy: LyricsOnlineStrategy
}

export const LYRICS_CHANGED_EVENT = 'lyrics://changed'
export const LYRICS_DIAGNOSTICS_CHANGED_EVENT = 'lyrics://diagnostics-changed'

/** 读取当前歌词快照。 */
export function getCurrentLyrics(): Promise<LyricsSnapshot> {
  return invoke<LyricsSnapshot>('get_current_lyrics')
}

/** 更新歌词偏好并让后端同步运行时状态。 */
export function setLyricsPreferences(preferences: LyricsPreferences): Promise<void> {
  return invoke('set_lyrics_preferences', {
    enabled: preferences.enabled,
    allowOnline: preferences.allowOnline,
    onlineStrategy: preferences.onlineStrategy,
  })
}
