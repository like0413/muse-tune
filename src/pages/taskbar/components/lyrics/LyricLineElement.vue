<script setup lang="ts">
import { useElementSize } from '@vueuse/core'
import type { CSSProperties, DeepReadonly } from 'vue'

import type { LyricLine } from '@/features/lyrics/types'
import {
  resolveLineProgress,
  resolveScrollProgress,
  resolveWordGradient,
  resolveWordHighlightState,
  type WordHighlightState,
} from '@/features/lyrics/word-highlight'
import { useReducedMotionPreference } from '@/features/motion/useReducedMotionPreference'
import type { TaskbarLyricsAlignment } from '@/features/settings/lyrics'

/** 第二行字号比第一行小 2px，晋升到第一行时用缩放补出这段“由小变大”。 */
const SECONDARY_FONT_SIZE_OFFSET = 2

const alignmentClasses: Record<TaskbarLyricsAlignment, string> = {
  left: 'text-left',
  center: 'text-center',
  right: 'text-right',
}

/** 缩放原点跟随对齐方式，保证放大过程中文字不会横向漂移。 */
const alignmentOrigins: Record<TaskbarLyricsAlignment, string> = {
  left: 'left center',
  center: 'center',
  right: 'right center',
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
const reducedMotion = useReducedMotionPreference()
const { width: viewportWidth } = useElementSize(viewport)
// 测量实际渲染的逐字内容，宽度与字号变化时由 ResizeObserver 同步滚动边界。
const { width: textWidth } = useElementSize(textMeasure)

/** 当前歌词行的播放比例；异常时间范围的处理见 `resolveLineProgress`。 */
const lineProgress = computed(() =>
  resolveLineProgress(props.line.startMs, props.line.endMs, props.positionMs),
)

/** 仅在歌词确实溢出时，于播放中段平滑滚动到行尾。 */
const overflowDistance = computed(() => Math.max(0, textWidth.value - viewportWidth.value))
const scrollProgress = computed(() => resolveScrollProgress(lineProgress.value))
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
  transformOrigin: alignmentOrigins[props.alignment],
  '--lyric-row-grow-from': `${(props.fontSize - SECONDARY_FONT_SIZE_OFFSET) / props.fontSize}`,
}))
const trackClass = computed(() => [
  alignmentClasses[props.alignment],
  overflowDistance.value > 0 ? 'w-max will-change-transform' : 'w-full',
  // 位置更新只有 20 FPS，用短线性过渡让合成器把位移插值到显示刷新率，避免滚动逐帧跳动。
  overflowDistance.value > 0 && !reducedMotion.value ? 'lyric-line-scrolling' : '',
])

/** 只有原文主行参与逐字高亮，翻译和下一句保持纯文本展示。 */
const renderWords = computed(() =>
  props.wordHighlight && props.text === props.line.text ? props.line.words : [],
)

/** 逐字高亮状态；非主行不参与，避免翻译行被当前词的渐变染色。 */
const highlight = computed<WordHighlightState>(() =>
  props.primary
    ? resolveWordHighlightState(renderWords.value, props.positionMs)
    : { completedCount: 0, activeIndex: -1 },
)
const activeWordIndex = computed(() => highlight.value.activeIndex)

/** 只为当前播放词生成动态渐变变量，其余词复用静态 CSS。 */
const activeWordStyle = computed<CSSProperties | undefined>(() => {
  const word = renderWords.value[activeWordIndex.value]
  if (!word) return undefined
  const gradient = resolveWordGradient(props.positionMs, word)
  return {
    '--lyric-word-gradient-start': `${gradient.progressStart}%`,
    '--lyric-word-gradient-center': `${gradient.progressCenter}%`,
    '--lyric-word-gradient-end': `${gradient.progressEnd}%`,
    '--lyric-word-gradient-start-color': `color-mix(in srgb, var(--lyric-played-color) ${gradient.startStrength}%, var(--lyric-unplayed-color))`,
    '--lyric-word-gradient-center-color': `color-mix(in srgb, var(--lyric-played-color) ${gradient.centerStrength}%, var(--lyric-unplayed-color))`,
    '--lyric-word-gradient-end-color': `color-mix(in srgb, var(--lyric-played-color) ${gradient.endStrength}%, var(--lyric-unplayed-color))`,
  } as CSSProperties
})

/** 返回稳定类名，避免为非活动词创建样式对象。 */
function wordStateClass(index: number) {
  if (index < highlight.value.completedCount) return 'lyric-word-completed'
  if (index === highlight.value.activeIndex) return 'lyric-word-active'
  return 'lyric-word-pending'
}

/**
 * 晋升到第一行时播放一次缩放动画：字号与行高已直接对齐到第一行大小，
 * 缩放把“由小变大”补回来。动画只跑 transform，不插值 font-size，
 * 因此文字不会逐帧重排，逐字渐变也不用每帧重画。
 */
const growing = shallowRef(false)
watch(
  () => props.primary,
  (primary, previouslyPrimary) => {
    if (!primary || previouslyPrimary || !props.animated) return
    // 先复位再于下一帧加类，连续两次晋升都能重新播放。
    growing.value = false
    void nextTick(() => (growing.value = true))
  },
)
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
      :class="{ 'lyric-row-growing': growing }"
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
/* 叠加在 20 FPS 的进度更新之上：时长略大于更新间隔，既不留空隙也不会感到迟滞。 */
.lyric-line-scrolling {
  transition: transform 80ms linear;
}

/* 晋升到第一行：字号与行高已直接对齐，这里只用整体缩放补出“由小变大”。 */
.lyric-row-growing {
  animation: lyric-row-grow var(--lyric-duration-ms) ease-out;
}

@keyframes lyric-row-grow {
  from {
    transform: scale(var(--lyric-row-grow-from));
  }
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
