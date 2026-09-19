import { useIntervalFn, useRafFn } from '@vueuse/core'
import type { DeepReadonly } from 'vue'

import type { MediaPlaybackStatus, MediaTimeline } from './types'

/**
 * 在播放器时间线事件之间平滑推演当前进度，并在新事件到达时重新校准。
 * 本地仅负责显示，不反向修改播放器发布的数据。
 */
export function useMediaProgress(
  timeline: DeepReadonly<Ref<MediaTimeline | null>>,
  playbackStatus: Readonly<Ref<MediaPlaybackStatus>>,
  active: Readonly<Ref<boolean>> = ref(true),
  smooth: Readonly<Ref<boolean>> = active,
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

  const { pause: pauseRaf, resume: resumeRaf } = useRafFn(
    ({ timestamp }) => updatePosition(timestamp),
    {
      immediate: false,
      // 逐字歌词仍需连续更新，但 20 FPS 已足够保持渐变平滑。
      fpsLimit: 20,
    },
  )
  const { pause: pauseInterval, resume: resumeInterval } = useIntervalFn(
    () => updatePosition(),
    // 进度条和频谱进度边界使用独立的 1 FPS 低频更新。
    1000,
    { immediate: false },
  )

  /** 按当前视觉消费者选择平滑 RAF、低频定时器或完全休眠。 */
  function synchronizeLoop() {
    pauseRaf()
    pauseInterval()
    if (!active.value || !timeline.value || playbackStatus.value !== 'playing') return
    if (smooth.value) resumeRaf()
    else resumeInterval()
  }

  /** 重新设置外推锚点，避免切换平滑与低频模式时产生跳变。 */
  function resynchronizePosition() {
    const now = performance.now()
    anchorPositionMs = positionMs.value
    anchorTime = now
    updatePosition(now)
  }

  watch(
    timeline,
    (value) => {
      anchorPositionMs = value?.positionMs ?? 0
      anchorTime = performance.now()
      updatePosition(anchorTime)
      synchronizeLoop()
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
      synchronizeLoop()
    },
    { immediate: true },
  )

  watch(
    active,
    (enabled) => {
      if (!enabled) {
        pauseRaf()
        pauseInterval()
        return
      }
      resynchronizePosition()
      synchronizeLoop()
    },
    { immediate: true },
  )

  watch(
    smooth,
    () => {
      resynchronizePosition()
      synchronizeLoop()
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
