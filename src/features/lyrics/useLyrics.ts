import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import { reportBackgroundFailure } from '@/features/feedback/errors'

import { getCurrentLyrics, LYRICS_CHANGED_EVENT } from './client'
import type { LyricsSnapshot } from './types'

const EMPTY_LYRICS: LyricsSnapshot = {
  trackKey: null,
  status: 'unavailable',
  source: null,
  precision: null,
  lines: [],
  errorReason: null,
}

/** 同步 Rust 歌词服务的低频快照；播放进度由媒体 composable 单独提供。 */
export function useLyrics(enabled: Readonly<Ref<boolean>> = ref(true)) {
  const lyrics = shallowRef<LyricsSnapshot>(EMPTY_LYRICS)
  let receivedEvent = false
  let disposed = false
  // 初始化代际号：关闭再开启会重跑一遍订阅与补取，代际号让上一代的监听与读取结果失效。
  let initializationId = 0
  let unlisten: UnlistenFn | undefined

  /** 先监听后补取缓存，避免窗口初始化期间漏掉解析完成事件。 */
  async function enable() {
    const currentInitializationId = ++initializationId
    // 上一代遗留的到达标记会让本代的初值读取被误判为过期，因此每代重新计数。
    receivedEvent = false
    try {
      const stopListener = await listen<LyricsSnapshot>(LYRICS_CHANGED_EVENT, ({ payload }) => {
        receivedEvent = true
        lyrics.value = payload
      })
      if (disposed || !enabled.value || currentInitializationId !== initializationId) {
        stopListener()
        return
      }
      unlisten = stopListener
      const initial = await getCurrentLyrics()
      if (
        !disposed &&
        enabled.value &&
        currentInitializationId === initializationId &&
        !receivedEvent
      ) {
        lyrics.value = initial
      }
    } catch (error) {
      reportBackgroundFailure('初始化歌词状态失败', error)
    }
  }

  watch(
    enabled,
    (value) => {
      if (value) {
        void enable()
        return
      }
      initializationId += 1
      unlisten?.()
      unlisten = undefined
      lyrics.value = EMPTY_LYRICS
    },
    { immediate: true },
  )

  onUnmounted(() => {
    disposed = true
    initializationId += 1
    unlisten?.()
  })

  return {
    lyrics: readonly(lyrics),
  }
}
