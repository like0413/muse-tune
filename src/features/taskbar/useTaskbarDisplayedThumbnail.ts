import { useTimeoutFn } from '@vueuse/core'
import type { Ref } from 'vue'

import { getThumbnailUpdateDebounceMs } from '@/features/media/players'
import type { MediaSessionSnapshot } from '@/features/media/types'

export interface DisplayedThumbnail {
  source: string
  image: HTMLImageElement
}

/** 共享封面预加载、防抖与短暂空值保留，供封面和背景使用同一张已就绪图片。 */
export function useTaskbarDisplayedThumbnail(session: Readonly<Ref<MediaSessionSnapshot | null>>) {
  const thumbnail = shallowRef<DisplayedThumbnail | null>(null)
  const updateDelayMs = shallowRef(0)
  let requestId = 0
  let pendingSource: string | null = null
  const { start: scheduleClear, stop: cancelClear } = useTimeoutFn(
    () => (thumbnail.value = null),
    500,
    { immediate: false },
  )
  const { start: scheduleUpdate, stop: cancelUpdate } = useTimeoutFn(
    () => void preload(pendingSource),
    updateDelayMs,
    { immediate: false },
  )

  /** 仅在新图片解码完成后发布，避免切歌时显示占位符。 */
  async function preload(source: string | null) {
    const currentRequest = ++requestId
    if (!source) {
      if (session.value) scheduleClear()
      else thumbnail.value = null
      return
    }

    cancelClear()
    if (thumbnail.value?.source === source) return
    const image = new Image()
    image.decoding = 'async'
    image.src = source
    try {
      await image.decode()
    } catch {
      if (currentRequest === requestId) scheduleClear()
      return
    }
    if (currentRequest === requestId) thumbnail.value = { source, image }
  }

  /** 按播放器规则合并切歌时连续发布的候选封面。 */
  function update(player: MediaSessionSnapshot['player'] | null, source: string | null) {
    cancelUpdate()
    const delay = player ? getThumbnailUpdateDebounceMs(player) : 0
    if (delay <= 0 || !source) {
      void preload(source)
      return
    }
    pendingSource = source
    updateDelayMs.value = delay
    scheduleUpdate()
  }

  watch(
    [() => session.value?.player ?? null, () => session.value?.metadata.thumbnailDataUrl ?? null],
    ([player, source]) => update(player, source),
    { immediate: true },
  )

  onUnmounted(() => {
    requestId += 1
    cancelClear()
    cancelUpdate()
  })

  return computed(() => thumbnail.value)
}
