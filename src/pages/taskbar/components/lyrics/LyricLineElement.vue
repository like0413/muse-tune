<script setup lang="ts">
import { useElementSize } from '@vueuse/core'
import type { CSSProperties, DeepReadonly } from 'vue'

import type { LyricLine } from '@/features/lyrics/types'
import type { TaskbarLyricsAlignment } from '@/features/settings/lyrics'

const SCROLL_START_PROGRESS = 0.28
const SCROLL_END_PROGRESS = 0.88
const WORD_GRADIENT_HALF_WIDTH = 12
const CENTER_COLOR_STRENGTH = 50

const alignmentClasses: Record<TaskbarLyricsAlignment, string> = {
  left: 'text-left',
  center: 'text-center',
  right: 'text-right',
}

const props = defineProps<{
  line: DeepReadonly<LyricLine>
  text: string
  positionMs: number
  primary: boolean
  wordHighlight: boolean
  animated: boolean
  alignment: TaskbarLyricsAlignment
  fontSize: number
  lineHeight: number
  rowTop: number
}>()

const viewport = useTemplateRef<HTMLElement>('viewport')
const textMeasure = useTemplateRef<HTMLElement>('textMeasure')
const { width: viewportWidth } = useElementSize(viewport)
// 测量实际渲染的逐字内容，宽度与字号变化时由 ResizeObserver 同步滚动边界。
const { width: textWidth } = useElementSize(textMeasure)

/** 计算当前歌词行的播放比例，异常时间范围直接保持起点。 */
const lineProgress = computed(() => {
  const duration = props.line.endMs - props.line.startMs
  if (duration <= 0) return 0
  return Math.min(1, Math.max(0, (props.positionMs - props.line.startMs) / duration))
})

/** 仅在歌词确实溢出时，于播放中段平滑滚动到行尾。 */
const overflowDistance = computed(() => Math.max(0, textWidth.value - viewportWidth.value))
const scrollProgress = computed(() =>
  Math.min(
    1,
    Math.max(
      0,
      (lineProgress.value - SCROLL_START_PROGRESS) / (SCROLL_END_PROGRESS - SCROLL_START_PROGRESS),
    ),
  ),
)
const trackStyle = computed<CSSProperties>(() => ({
  transform:
    overflowDistance.value > 0
      ? `translate3d(${-overflowDistance.value * scrollProgress.value}px, 0, 0)`
      : undefined,
}))
const rowStyle = computed<CSSProperties>(() => ({
  '--lyric-row-top': `${props.rowTop}px`,
  color: props.primary ? 'var(--lyric-played-color)' : 'var(--lyric-unplayed-color)',
  height: `${props.lineHeight}px`,
}))
const contentStyle = computed<CSSProperties>(() => ({
  fontSize: `${props.fontSize}px`,
  height: `${props.lineHeight}px`,
  lineHeight: `${props.lineHeight}px`,
}))
const trackClass = computed(() => [
  alignmentClasses[props.alignment],
  overflowDistance.value > 0 ? 'w-max will-change-transform' : 'w-full',
])

/** 只有原文主行参与逐字高亮，翻译和下一句保持纯文本展示。 */
const renderWords = computed(() =>
  props.wordHighlight && props.text === props.line.text ? props.line.words : [],
)

/** 二分定位已播放词边界，避免每次进度刷新都复制整行逐字数组。 */
const completedWordCount = computed(() => {
  if (!props.primary) return 0
  const words = renderWords.value
  let left = 0
  let right = words.length
  while (left < right) {
    const middle = Math.floor((left + right) / 2)
    if (words[middle]!.endMs <= props.positionMs) left = middle + 1
    else right = middle
  }
  return left
})

/** 当前播放词；词间空隙不错误地延续上一词渐变。 */
const activeWordIndex = computed(() => {
  if (!props.primary) return -1
  const index = completedWordCount.value
  const word = renderWords.value[index]
  return word && props.positionMs > word.startMs && props.positionMs < word.endMs ? index : -1
})

/** 只为当前播放词生成动态渐变变量，其余词复用静态 CSS。 */
const activeWordStyle = computed<CSSProperties | undefined>(() => {
  const word = renderWords.value[activeWordIndex.value]
  if (!word) return undefined
  const duration = Math.max(1, word.endMs - word.startMs)
  const progress = Math.min(100, Math.max(0, ((props.positionMs - word.startMs) / duration) * 100))
  const start = Math.max(0, progress - WORD_GRADIENT_HALF_WIDTH)
  const end = Math.min(100, progress + WORD_GRADIENT_HALF_WIDTH)
  const enteringRatio = Math.min(1, progress / WORD_GRADIENT_HALF_WIDTH)
  const leavingRatio = Math.max(
    0,
    (progress - (100 - WORD_GRADIENT_HALF_WIDTH)) / WORD_GRADIENT_HALF_WIDTH,
  )
  const startStrength = 100 * enteringRatio
  const centerStrength =
    progress < WORD_GRADIENT_HALF_WIDTH
      ? CENTER_COLOR_STRENGTH * enteringRatio
      : CENTER_COLOR_STRENGTH + (100 - CENTER_COLOR_STRENGTH) * leavingRatio
  const endStrength = 100 * leavingRatio
  return {
    '--lyric-word-gradient-start': `${start}%`,
    '--lyric-word-gradient-center': `${progress}%`,
    '--lyric-word-gradient-end': `${end}%`,
    '--lyric-word-gradient-start-color': `color-mix(in srgb, var(--lyric-played-color) ${startStrength}%, var(--lyric-unplayed-color))`,
    '--lyric-word-gradient-center-color': `color-mix(in srgb, var(--lyric-played-color) ${centerStrength}%, var(--lyric-unplayed-color))`,
    '--lyric-word-gradient-end-color': `color-mix(in srgb, var(--lyric-played-color) ${endStrength}%, var(--lyric-unplayed-color))`,
  } as CSSProperties
})

/** 返回稳定类名，避免为非活动词创建样式对象。 */
function wordStateClass(index: number) {
  if (index < completedWordCount.value) return 'lyric-word-completed'
  if (index === activeWordIndex.value) return 'lyric-word-active'
  return 'lyric-word-pending'
}
</script>

<template>
  <div
    class="flex w-full min-w-0 items-center overflow-hidden font-medium"
    :style="rowStyle"
    aria-hidden="true"
  >
    <div
      ref="viewport"
      class="relative w-full overflow-hidden"
      :class="{ 'lyric-content-animated': animated }"
      :style="contentStyle"
    >
      <div class="lyric-line-track whitespace-pre" :class="trackClass" :style="trackStyle">
        <span ref="textMeasure" class="inline-block w-max align-top">
          <template v-if="renderWords.length > 0">
            <span
              v-for="(word, index) in renderWords"
              :key="`${word.startMs}-${index}`"
              class="lyric-word"
              :class="wordStateClass(index)"
              :style="index === activeWordIndex ? activeWordStyle : undefined"
              >{{ word.text }}</span
            >
          </template>
          <span v-else class="lyric-plain-text">{{ text }}</span>
        </span>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 动画时长由 LyricsElement 统一下发，保证行换位与字号变化同步。 */
.lyric-content-animated {
  transition:
    height var(--lyric-duration-ms) ease-out,
    line-height var(--lyric-duration-ms) ease-out,
    font-size var(--lyric-duration-ms) ease-out;
}

.lyric-word {
  white-space: pre;
  background-clip: text;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.lyric-word-pending {
  background-image: linear-gradient(var(--lyric-unplayed-color), var(--lyric-unplayed-color));
}

.lyric-word-completed {
  background-image: linear-gradient(var(--lyric-played-color), var(--lyric-played-color));
}

.lyric-word-active {
  background-image: linear-gradient(
    to right,
    var(--lyric-word-gradient-start-color) 0%,
    var(--lyric-word-gradient-start-color) var(--lyric-word-gradient-start),
    var(--lyric-word-gradient-center-color) var(--lyric-word-gradient-center),
    var(--lyric-word-gradient-end-color) var(--lyric-word-gradient-end),
    var(--lyric-word-gradient-end-color) 100%
  );
}
</style>
