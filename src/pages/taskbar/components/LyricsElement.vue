<script setup lang="ts">
import type { CSSProperties, DeepReadonly } from 'vue'

import { resolveTaskbarLyricsAppearance } from '@/features/lyrics/appearance'
import {
  LINE_TRANSITION_DURATION_MS,
  resolveCurrentLineIndex,
  resolveLayoutMetrics,
  resolveTransitionLeadMs,
  selectSecondaryContent,
  type SecondaryContent,
} from '@/features/lyrics/line-window'
import type { LyricLine, LyricsSnapshot } from '@/features/lyrics/types'
import { useReducedMotionPreference } from '@/features/motion/useReducedMotionPreference'
import type { TaskbarLyricsSettings } from '@/features/settings/lyrics'

import LyricLineElement from './lyrics/LyricLineElement.vue'

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

/** 二分定位当前行；换行触发点的提前规则见 `line-window.ts`。 */
const currentLineIndex = computed(() =>
  resolveCurrentLineIndex(
    props.lyrics.lines,
    props.positionMs,
    resolveTransitionLeadMs(lyricsAnimationEnabled.value, props.settings.animationPreRoll),
  ),
)

/**
 * 只在行号真正变化时更新。
 * `currentLineIndex` 依赖每帧变化的 `positionMs`，直接作为下游依赖会让行级数据与
 * 样式对象在播放中每帧重建；改由本引用驱动后，它们只在换行时重算。
 */
const activeLineIndex = shallowRef(-1)
watch(
  currentLineIndex,
  (index) => {
    activeLineIndex.value = index
  },
  { immediate: true },
)

/** 按当前设置挑出第二行内容；选择与回退规则见 `selectSecondaryContent`。 */
function currentSecondaryContent(index: number): SecondaryContent | undefined {
  const current = props.lyrics.lines[index]
  if (!current) return undefined
  return selectSecondaryContent(
    props.settings.secondaryLine,
    current,
    props.lyrics.lines[index + 1],
  )
}

/** 判断当前内容是否确实存在第二行，末句无翻译和下一句时恢复单行居中。 */
const hasSecondaryLine = computed(
  () =>
    props.settings.lineMode === 'double' && Boolean(currentSecondaryContent(activeLineIndex.value)),
)

/** 行高、字号与居中偏移；推导规则见 `resolveLayoutMetrics`。 */
const layoutMetrics = computed(() =>
  resolveLayoutMetrics(props.settings.fontSize, hasSecondaryLine.value),
)

/** 生成最多两行稳定标识的数据，使下一句能够准确移动到第一行槽位。 */
const displayLines = computed<DisplayLine[]>(() => {
  const index = activeLineIndex.value
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

  const secondary = currentSecondaryContent(index)
  if (secondary) {
    lines.push({
      key:
        secondary.kind === 'translation'
          ? `${trackKey}:${index}:translation`
          : `${trackKey}:${index + 1}:original`,
      line: secondary.line,
      text: secondary.text,
      primary: false,
      fontSize: layoutMetrics.value.secondaryFontSize,
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
