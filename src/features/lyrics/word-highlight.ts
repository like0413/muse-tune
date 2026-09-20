import { clamp } from 'es-toolkit'

import type { LyricWord } from './types'

/**
 * 歌词行的播放比例区间：滚动只在行中段发生，首尾留出静止时间，
 * 否则歌词刚出现就开始滑动，视觉上会很焦躁。
 */
export const SCROLL_START_PROGRESS = 0.28
export const SCROLL_END_PROGRESS = 0.88

/** 逐字高亮渐变在词两侧的过渡宽度，单位是词内进度的百分比。 */
const WORD_GRADIENT_HALF_WIDTH = 12
/** 当前词中心色的强度下限；词刚进入时也要能看出高亮正在推进。 */
const CENTER_COLOR_STRENGTH = 50

/** 当前歌词行的播放比例；时间范围异常时保持起点。 */
export function resolveLineProgress(startMs: number, endMs: number, positionMs: number): number {
  const duration = endMs - startMs
  if (duration <= 0) return 0
  return clamp((positionMs - startMs) / duration, 0, 1)
}

/** 把行进度映射为滚动比例：中段线性推进，两端静止。 */
export function resolveScrollProgress(lineProgress: number): number {
  const span = SCROLL_END_PROGRESS - SCROLL_START_PROGRESS
  return clamp((lineProgress - SCROLL_START_PROGRESS) / span, 0, 1)
}

export interface WordHighlightState {
  /** 已播放完的词个数，用于给前缀词打 completed 状态。 */
  completedCount: number
  /** 正在播放的词下标；词间空隙或整行已播完时为 -1。 */
  activeIndex: number
}

/**
 * 定位当前词的逐字高亮状态。
 *
 * 只有位置**严格落在**某个词的时间区间内才算“正在播放”：词与词之间的空隙不延续
 * 上一词的渐变，否则间隙里会残留一段正在推进的高亮。
 */
export function resolveWordHighlightState(
  words: readonly LyricWord[],
  positionMs: number,
): WordHighlightState {
  const completedCount = findCompletedWordCount(words, positionMs)
  const word = words[completedCount]
  const activeIndex =
    word && positionMs > word.startMs && positionMs < word.endMs ? completedCount : -1
  return { completedCount, activeIndex }
}

/**
 * 二分定位已播放词边界，避免每次进度刷新都复制或线性扫描整行逐字数组。
 * 依赖逐字数组按时间升序且互不重叠。
 */
function findCompletedWordCount(words: readonly LyricWord[], positionMs: number): number {
  let left = 0
  let right = words.length
  while (left < right) {
    const middle = Math.floor((left + right) / 2)
    if (words[middle]!.endMs <= positionMs) left = middle + 1
    else right = middle
  }
  return left
}

/** 逐字渐变的数值形态；组件负责把它拼成 CSS 变量。 */
export interface WordGradient {
  progressStart: number
  progressCenter: number
  progressEnd: number
  startStrength: number
  centerStrength: number
  endStrength: number
}

/** 计算当前词的三段渐变位置与已播放色强度。 */
export function resolveWordGradient(positionMs: number, word: LyricWord): WordGradient {
  // 零长度词在真实数据里会出现（占位或解析异常），兜底避免除零产生 NaN。
  const duration = Math.max(1, word.endMs - word.startMs)
  const progress = clamp(((positionMs - word.startMs) / duration) * 100, 0, 100)
  // 进入阶段：渐变前沿从词首推进到中心。
  const enteringRatio = Math.min(1, progress / WORD_GRADIENT_HALF_WIDTH)
  // 离开阶段：渐变后沿从中心推进到词尾。
  const leavingRatio = Math.max(
    0,
    (progress - (100 - WORD_GRADIENT_HALF_WIDTH)) / WORD_GRADIENT_HALF_WIDTH,
  )

  return {
    progressStart: Math.max(0, progress - WORD_GRADIENT_HALF_WIDTH),
    progressCenter: progress,
    progressEnd: Math.min(100, progress + WORD_GRADIENT_HALF_WIDTH),
    startStrength: 100 * enteringRatio,
    centerStrength:
      progress < WORD_GRADIENT_HALF_WIDTH
        ? CENTER_COLOR_STRENGTH * enteringRatio
        : CENTER_COLOR_STRENGTH + (100 - CENTER_COLOR_STRENGTH) * leavingRatio,
    endStrength: 100 * leavingRatio,
  }
}
