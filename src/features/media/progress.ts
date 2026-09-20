import { clamp } from 'es-toolkit'

import type { MediaTimeline } from './types'

/**
 * 在播放器时间线事件之间外推当前位置。
 *
 * 播放器只在状态变化时推送时间线，界面需要靠锚点 + 倍速自行推演；
 * 推演结果始终钳制在时间线区间内，避免播放器报出偏短时长时进度条溢出。
 */
export function extrapolatePosition(
  timeline: MediaTimeline | null,
  anchorPositionMs: number,
  elapsedMs: number,
  isPlaying: boolean,
): number {
  if (!timeline) return 0
  const elapsed = isPlaying ? elapsedMs : 0
  return clamp(
    anchorPositionMs + elapsed * timeline.playbackRate,
    timeline.startTimeMs,
    timeline.endTimeMs,
  )
}

/** 当前播放位置在整首歌里的百分比；时长无效时返回 0。 */
export function computeProgressPercent(positionMs: number, timeline: MediaTimeline | null): number {
  if (!timeline) return 0
  const duration = timeline.endTimeMs - timeline.startTimeMs
  if (duration <= 0) return 0
  return clamp(((positionMs - timeline.startTimeMs) / duration) * 100, 0, 100)
}
