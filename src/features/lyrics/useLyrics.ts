import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import type { LyricsSnapshot } from './types'

const LYRICS_CHANGED_EVENT = 'lyrics://changed'
const EMPTY_LYRICS: LyricsSnapshot = {
  trackKey: null,
  status: 'unavailable',
  source: null,
  precision: null,
  lines: [],
  errorReason: null,
}

/** 同步 Rust 歌词服务的低频快照；播放进度由媒体 composable 单独提供。 */
export function useLyrics() {
  const lyrics = shallowRef<LyricsSnapshot>(EMPTY_LYRICS)
  let receivedEvent = false
  let disposed = false
  let unlisten: UnlistenFn | undefined

  /** 先监听后补取缓存，避免窗口初始化期间漏掉解析完成事件。 */
  async function initialize() {
    try {
      const stopListener = await listen<LyricsSnapshot>(LYRICS_CHANGED_EVENT, ({ payload }) => {
        receivedEvent = true
        lyrics.value = payload
      })
      if (disposed) {
        stopListener()
        return
      }
      unlisten = stopListener
      const initial = await invoke<LyricsSnapshot>('get_current_lyrics')
      if (!disposed && !receivedEvent) lyrics.value = initial
    } catch (error) {
      console.error('初始化歌词状态失败', error)
    }
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })

  return {
    lyrics: readonly(lyrics),
  }
}
