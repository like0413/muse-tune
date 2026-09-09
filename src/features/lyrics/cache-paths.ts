import { invoke } from '@tauri-apps/api/core'

import type { MediaPlayer } from '@/features/media/types'

import type { LyricsCachePathState } from './types'

/** 读取四家播放器的歌词目录发现状态。 */
export function getLyricsCachePaths(): Promise<LyricsCachePathState[]> {
  return invoke('get_lyrics_cache_paths')
}

/** 设置目录覆盖；传入 null 时恢复播放器配置自动发现。 */
export function setLyricsCachePathOverride(
  player: MediaPlayer,
  path: string | null,
): Promise<LyricsCachePathState[]> {
  return invoke('set_lyrics_cache_path_override', { player, path })
}
