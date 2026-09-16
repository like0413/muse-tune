<script setup lang="ts">
import type { CSSProperties, DeepReadonly } from 'vue'

import { resolveTaskbarLyricsAppearance } from '@/features/lyrics/appearance'
import type { LyricLine, LyricsSnapshot } from '@/features/lyrics/types'
import { useReducedMotionPreference } from '@/features/motion/useReducedMotionPreference'
import type { TaskbarLyricsSettings } from '@/features/settings/lyrics'

import LyricLineElement from './lyrics/LyricLineElement.vue'

const AVAILABLE_HEIGHT_PX = 40
const TRACK_INFO_TITLE_LINE_HEIGHT_PX = 17.5
const TRACK_INFO_ARTIST_LINE_HEIGHT_PX = 15
const LINE_TRANSITION_DURATION_MS = 300
// 300ms 动画加 30ms 刷新余量，使切换在人声时间戳前完成。
const LINE_TRANSITION_LEAD_MS = LINE_TRANSITION_DURATION_MS + 30

const props = defineProps<{
  lyrics: DeepReadonly<LyricsSnapshot>
  positionMs: number
  settings: DeepReadonly<TaskbarLyricsSettings>
  themeColor: string
}>()

const reducedMotion = useReducedMotionPreference()
/** 减少动态效果时彻底跳过 Vue 过渡，避免零时长透明度状态产生闪烁。 */
const lyricsAnimationEnabled = computed(
  () => props.settings.animation !== 'none' && !reducedMotion.value,
)

interface DisplayLine {
  key: string
  line: DeepReadonly<LyricLine>
  text: string
  primary: boolean
  fontSize: number
  lineHeight: number
  rowTop: number
}

/** 使用二分查找定位当前行；启用动画时把切行触发点整体提前。 */
const currentLineIndex = computed(() => {
  const lines = props.lyrics.lines
  if (lines.length === 0) return -1
  const transitionLeadMs =
    lyricsAnimationEnabled.value && props.settings.animationPreRoll ? LINE_TRANSITION_LEAD_MS : 0
  let left = 0
  let right = lines.length - 1
  let matched = -1
  while (left <= right) {
    const middle = Math.floor((left + right) / 2)
    if (lines[middle]!.startMs <= props.positionMs + transitionLeadMs) {
      matched = middle
      left = middle + 1
    } else {
      right = middle - 1
    }
  }
  // 前奏阶段提前展示第一句未播放歌词，避免歌词层已经启用却完全空白。
  return Math.max(0, matched)
})

/** 判断当前内容是否确实存在第二行，末句无翻译和下一句时恢复单行居中。 */
const hasSecondaryLine = computed(() => {
  if (props.settings.lineMode !== 'double') return false
  const index = currentLineIndex.value
  const current = props.lyrics.lines[index]
  return Boolean(current && selectSecondaryContent(current, props.lyrics.lines[index + 1]))
})

/** 按设置选择第二行内容，只有“翻译优先”会在缺失时回退下一句。 */
function selectSecondaryContent(
  current: DeepReadonly<LyricLine>,
  next: DeepReadonly<LyricLine> | undefined,
) {
  const translation = current.translation
    ? { kind: 'translation' as const, line: current, text: current.translation }
    : undefined
  const nextLine = next ? { kind: 'next' as const, line: next, text: next.text } : undefined
  switch (props.settings.secondaryLine) {
    case 'translation_only':
      return translation
    case 'next':
      return nextLine
    case 'translation_or_next':
      return translation ?? nextLine
  }
}

/** 14px 严格复用普通歌曲信息的两种行高，其余字号采用紧凑且不会裁切的行高。 */
const layoutMetrics = computed(() => {
  const usesTwoLines = hasSecondaryLine.value
  const secondaryFontSize = Math.max(10, props.settings.fontSize - 2)
  const primaryLineHeight =
    props.settings.fontSize === 14 ? TRACK_INFO_TITLE_LINE_HEIGHT_PX : props.settings.fontSize + 2
  const secondaryLineHeight =
    props.settings.fontSize === 14 ? TRACK_INFO_ARTIST_LINE_HEIGHT_PX : secondaryFontSize + 2
  const blockHeight = usesTwoLines ? primaryLineHeight + secondaryLineHeight : primaryLineHeight
  return {
    primaryLineHeight,
    secondaryLineHeight,
    transitionStep: primaryLineHeight,
    blockHeight,
    blockTop: (AVAILABLE_HEIGHT_PX - blockHeight) / 2,
  }
})

/** 生成最多两行稳定标识的数据，使下一句能够准确移动到第一行槽位。 */
const displayLines = computed<DisplayLine[]>(() => {
  const index = currentLineIndex.value
  const current = props.lyrics.lines[index]
  if (!current) return []
  const trackKey = props.lyrics.trackKey ?? 'unknown'
  const lines: DisplayLine[] = [
    {
      key: `${trackKey}:${index}:original`,
      line: current,
      text: current.text,
      primary: true,
      fontSize: props.settings.fontSize,
      lineHeight: layoutMetrics.value.primaryLineHeight,
      rowTop: layoutMetrics.value.blockTop,
    },
  ]
  if (props.settings.lineMode === 'single') return lines

  const next = props.lyrics.lines[index + 1]
  const secondary = selectSecondaryContent(current, next)
  if (secondary) {
    lines.push({
      key:
        secondary.kind === 'translation'
          ? `${trackKey}:${index}:translation`
          : `${trackKey}:${index + 1}:original`,
      line: secondary.line,
      text: secondary.text,
      primary: false,
      fontSize: Math.max(10, props.settings.fontSize - 2),
      lineHeight: layoutMetrics.value.secondaryLineHeight,
      rowTop: layoutMetrics.value.blockTop + layoutMetrics.value.primaryLineHeight,
    })
  }
  return lines
})

const transitionName = computed(() => `lyrics-${props.settings.animation}`)

/** 将配色方案与动态主题色解析为单一组 CSS 变量。 */
const appearance = computed(() => resolveTaskbarLyricsAppearance(props.settings, props.themeColor))

/** 小字号保持紧凑间距，大字号自动收紧，双行总高度始终不超过 40px。 */
const displayStyle = computed<CSSProperties>(() => {
  const { transitionStep, blockTop } = layoutMetrics.value
  return {
    '--lyric-line-step': `${transitionStep}px`,
    '--lyric-duration-ms': `${LINE_TRANSITION_DURATION_MS}ms`,
    '--lyric-played-color': appearance.value.playedColor,
    '--lyric-unplayed-color': appearance.value.unplayedColor,
    fontFamily: appearance.value.fontFamily,
    paddingTop: `${blockTop}px`,
  } as CSSProperties
})
</script>

<template>
  <TransitionGroup
    tag="div"
    class="lyric-display relative flex h-full min-w-0 flex-1 flex-col justify-start overflow-hidden"
    :name="transitionName"
    :css="lyricsAnimationEnabled"
    :style="displayStyle"
  >
    <LyricLineElement
      v-for="line in displayLines"
      :key="line.key"
      :line="line.line"
      :text="line.text"
      :position-ms="positionMs"
      :primary="line.primary"
      :word-highlight="settings.wordHighlight"
      :animated="lyricsAnimationEnabled"
      :alignment="settings.alignment"
      :font-size="line.fontSize"
      :line-height="line.lineHeight"
      :row-top="line.rowTop"
      :class="line.primary ? 'lyric-primary-row' : 'lyric-secondary-row'"
      class="shrink-0"
    />
  </TransitionGroup>
</template>

<style scoped>
.lyrics-up-move,
.lyrics-up-enter-active,
.lyrics-up-leave-active {
  transition:
    transform var(--lyric-duration-ms) ease-out,
    opacity var(--lyric-duration-ms) ease;
}

/* 离开行固定在原槽位，不参与重排，因此不会在动画末尾额外上移。 */
.lyrics-up-leave-active {
  position: absolute;
  top: var(--lyric-row-top);
  width: 100%;
}

.lyrics-up-enter-from {
  opacity: 0;
  transform: translateY(var(--lyric-line-step));
}

/* 第二行保留位移动画，但不改变透明度，避免出现过程中文字颜色逐渐加深。 */
.lyrics-up-enter-from.lyric-secondary-row {
  opacity: 1;
}

.lyrics-up-leave-to {
  opacity: 0;
  transform: translateY(calc(-1 * var(--lyric-line-step)));
}
</style>
