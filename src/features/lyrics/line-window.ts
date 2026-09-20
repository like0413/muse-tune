import type { DeepReadonly } from 'vue'

import type { TaskbarLyricsSettings } from '@/features/settings/lyrics'

import type { LyricLine } from './types'

/** 歌词容器可用高度；双行内容连同居中偏移都必须放得下。 */
export const LYRICS_AVAILABLE_HEIGHT_PX = 40
/** 单行切换动画时长，同时作为 `--lyric-duration-ms` 暴露给样式。 */
export const LINE_TRANSITION_DURATION_MS = 300

/** 字号等于歌曲信息的标题字号时复用其行高，保持一致的行距节奏。 */
const TRACK_INFO_FONT_SIZE = 14
const TRACK_INFO_TITLE_LINE_HEIGHT_PX = 17.5
const TRACK_INFO_ARTIST_LINE_HEIGHT_PX = 15
/** 第二行字号相对第一行缩小 2px，并保底 10px。 */
const SECONDARY_FONT_SIZE_OFFSET_STEP = 2
const MIN_FONT_SIZE = 10
// 300ms 动画加 30ms 刷新余量，使切换在人声时间戳前完成。
const LINE_TRANSITION_LEAD_MS = LINE_TRANSITION_DURATION_MS + 30

/** 换行触发点的提前量；未开启动画或未开预滚时必须精确按时间戳切换。 */
export function resolveTransitionLeadMs(animated: boolean, animationPreRoll: boolean): number {
  return animated && animationPreRoll ? LINE_TRANSITION_LEAD_MS : 0
}

/**
 * 二分定位当前歌词行。
 *
 * 前奏阶段（位置早于第一句）返回 0 而不是 -1：歌词层已经启用却整块空白会让用户
 * 以为功能失效，提前展示第一句未播放歌词更符合预期。
 */
export function resolveCurrentLineIndex(
  lines: readonly DeepReadonly<LyricLine>[],
  positionMs: number,
  transitionLeadMs: number,
): number {
  if (lines.length === 0) return -1
  const threshold = positionMs + transitionLeadMs
  let left = 0
  let right = lines.length - 1
  let matched = -1
  while (left <= right) {
    const middle = Math.floor((left + right) / 2)
    if (lines[middle]!.startMs <= threshold) {
      matched = middle
      left = middle + 1
    } else {
      right = middle - 1
    }
  }
  return Math.max(0, matched)
}

export type SecondaryLinePolicy = TaskbarLyricsSettings['secondaryLine']

export interface SecondaryContent {
  kind: 'translation' | 'next'
  line: DeepReadonly<LyricLine>
  text: string
}

/** 按设置选择第二行内容，只有"翻译优先"会在缺失时回退到下一句。 */
export function selectSecondaryContent(
  policy: SecondaryLinePolicy,
  current: DeepReadonly<LyricLine>,
  next: DeepReadonly<LyricLine> | undefined,
): SecondaryContent | undefined {
  const translation = current.translation
    ? { kind: 'translation' as const, line: current, text: current.translation }
    : undefined
  const nextLine = next ? { kind: 'next' as const, line: next, text: next.text } : undefined
  switch (policy) {
    case 'translation_only':
      return translation
    case 'next':
      return nextLine
    case 'translation_or_next':
      return translation ?? nextLine
  }
}

export interface LyricsLayoutMetrics {
  primaryLineHeight: number
  secondaryFontSize: number
  secondaryLineHeight: number
  /** 两行槽位之间的位移，等于第一行行高。 */
  transitionStep: number
  blockHeight: number
  /** 内容块在可用高度内的垂直居中偏移。 */
  blockTop: number
}

/** 由字号与是否双行推导行高与居中偏移。 */
export function resolveLayoutMetrics(fontSize: number, usesTwoLines: boolean): LyricsLayoutMetrics {
  const secondaryFontSize = Math.max(MIN_FONT_SIZE, fontSize - SECONDARY_FONT_SIZE_OFFSET_STEP)
  const primaryLineHeight =
    fontSize === TRACK_INFO_FONT_SIZE ? TRACK_INFO_TITLE_LINE_HEIGHT_PX : fontSize + 2
  const secondaryLineHeight =
    fontSize === TRACK_INFO_FONT_SIZE ? TRACK_INFO_ARTIST_LINE_HEIGHT_PX : secondaryFontSize + 2
  const blockHeight = usesTwoLines ? primaryLineHeight + secondaryLineHeight : primaryLineHeight

  return {
    primaryLineHeight,
    secondaryFontSize,
    secondaryLineHeight,
    transitionStep: primaryLineHeight,
    blockHeight,
    blockTop: (LYRICS_AVAILABLE_HEIGHT_PX - blockHeight) / 2,
  }
}
