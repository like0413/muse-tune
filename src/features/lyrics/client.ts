import { invoke } from '@tauri-apps/api/core'

import type { MediaPlayer } from '@/features/media/types'

import type { LyricsChineseVariant, LyricsOnlineStrategy, LyricsSnapshot } from './types'

interface LyricsPreferences {
  enabled: boolean
  chineseVariant: LyricsChineseVariant
  allowOnline: boolean
  onlineStrategy: LyricsOnlineStrategy
  /** 生效的在线接口，按请求顺序排列；空数组表示不发起在线查询。 */
  onlineSources: MediaPlayer[]
}

export const LYRICS_CHANGED_EVENT = 'lyrics://changed'
export const LYRICS_DIAGNOSTICS_CHANGED_EVENT = 'lyrics://diagnostics-changed'

/** 读取当前歌词快照。 */
export function getCurrentLyrics(): Promise<LyricsSnapshot> {
  return invoke<LyricsSnapshot>('get_current_lyrics')
}

/** 复用原生 OpenCC 批量转换歌曲名、歌手等短文本。 */
export function convertChineseTexts(
  texts: string[],
  chineseVariant: LyricsChineseVariant,
): Promise<string[]> {
  return invoke<string[]>('convert_chinese_texts', { texts, chineseVariant })
}

/** 更新歌词偏好并让后端同步运行时状态。 */
export function setLyricsPreferences(preferences: LyricsPreferences): Promise<void> {
  return invoke('set_lyrics_preferences', {
    enabled: preferences.enabled,
    chineseVariant: preferences.chineseVariant,
    allowOnline: preferences.allowOnline,
    onlineStrategy: preferences.onlineStrategy,
    onlineSources: preferences.onlineSources,
  })
}
