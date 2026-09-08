import { useRafFn } from '@vueuse/core'
import type { DeepReadonly, Ref } from 'vue'

import type { MediaPlaybackStatus, MediaTimeline } from './types'

/**
 * 在播放器时间线事件之间平滑推演当前进度，并在新事件到达时重新校准。
 * 本地仅负责显示，不反向修改播放器发布的数据。
 */
export function useMediaProgress(
  timeline: DeepReadonly<Ref<MediaTimeline | null>>,
  playbackStatus: Readonly<Ref<MediaPlaybackStatus>>,
) {
  const positionMs = shallowRef(0)
  let anchorPositionMs = 0
  let anchorTime = performance.now()

  /** 依据当前锚点和播放器倍速计算位置，并限制在有效时间线内。 */
  function updatePosition(now = performance.now(), isPlaying = playbackStatus.value === 'playing') {
    const value = timeline.value
    if (!value) {
      positionMs.value = 0
      return
    }
    const elapsed = isPlaying ? now - anchorTime : 0
    positionMs.value = Math.min(
      value.endTimeMs,
      Math.max(value.startTimeMs, anchorPositionMs + elapsed * value.playbackRate),
    )
  }

  const { pause, resume } = useRafFn(({ timestamp }) => updatePosition(timestamp), {
    immediate: false,
    fpsLimit: 30,
  })

  watch(
    timeline,
    (value) => {
      anchorPositionMs = value?.positionMs ?? 0
      anchorTime = performance.now()
      updatePosition(anchorTime)
    },
    { immediate: true },
  )

  watch(
    playbackStatus,
    (status, previousStatus) => {
      const now = performance.now()
      if (previousStatus === 'playing') updatePosition(now, true)
      anchorPositionMs = positionMs.value
      anchorTime = now
      if (status === 'playing' && timeline.value) resume()
      else pause()
    },
    { immediate: true },
  )

  const progress = computed(() => {
    const value = timeline.value
    if (!value) return 0
    const duration = value.endTimeMs - value.startTimeMs
    if (duration <= 0) return 0
    return Math.min(100, Math.max(0, ((positionMs.value - value.startTimeMs) / duration) * 100))
  })

  return {
    positionMs: readonly(positionMs),
    progress,
  }
}
