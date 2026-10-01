import { useDocumentVisibility, useIntervalFn } from '@vueuse/core'
import type { DeepReadonly } from 'vue'

import { computeProgressPercent, extrapolatePosition } from './progress'
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
  const documentVisibility = useDocumentVisibility()
  let anchorPositionMs = 0
  let anchorTime = performance.now()

  /** 依据当前锚点和播放器倍速计算位置；外推与钳制规则见 `progress.ts`。 */
  function updatePosition(now = performance.now(), isPlaying = playbackStatus.value === 'playing') {
    positionMs.value = extrapolatePosition(
      timeline.value,
      anchorPositionMs,
      now - anchorTime,
      isPlaying,
    )
  }

  // 平滑模式仍维持 20 FPS；定时器直接按目标频率唤醒，避免高刷新率屏幕上 RAF 空转。
  const interval = computed(() => (smooth.value ? 50 : 1000))
  const { pause, resume } = useIntervalFn(() => updatePosition(), interval, { immediate: false })

  /** 只有可见消费者且正在播放时推进时钟。 */
  function synchronizeLoop() {
    pause()
    if (
      active.value &&
      documentVisibility.value === 'visible' &&
      timeline.value &&
      playbackStatus.value === 'playing'
    )
      resume()
  }

  /** 重新设置外推锚点，避免切换平滑与低频模式时产生跳变。 */
  function resynchronizePosition() {
    const now = performance.now()
    updatePosition(now)
    anchorPositionMs = positionMs.value
    anchorTime = now
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
    [active, documentVisibility],
    ([enabled, visibility]) => {
      if (!enabled || visibility !== 'visible') {
        pause()
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

  const progress = computed(() => computeProgressPercent(positionMs.value, timeline.value))

  return {
    positionMs: readonly(positionMs),
    progress,
  }
}
