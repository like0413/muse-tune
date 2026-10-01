<script setup lang="ts">
import { useElementSize } from '@vueuse/core'
import type { CSSProperties, DeepReadonly } from 'vue'

import type { LyricLine } from '@/features/lyrics/types'
import {
  resolveLineProgress,
  resolveScrollProgress,
  resolveWordHighlightState,
  type WordHighlightState,
} from '@/features/lyrics/word-highlight'
import { useReducedMotionPreference } from '@/features/motion/useReducedMotionPreference'
import type { TaskbarLyricsAlignment } from '@/features/settings/lyrics'
import { useTaskbarPlaybackClock } from '@/features/taskbar/playback-clock'

import LyricActiveWordElement from './LyricActiveWordElement.vue'

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
const { lyricsPositionMs } = useTaskbarPlaybackClock()
const { width: viewportWidth } = useElementSize(viewport)
// 测量实际渲染的逐字内容，宽度与字号变化时由 ResizeObserver 同步滚动边界。
const { width: textWidth } = useElementSize(textMeasure)

/** 当前歌词行的播放比例；异常时间范围的处理见 `resolveLineProgress`。 */
const lineProgress = computed(() =>
  resolveLineProgress(props.line.startMs, props.line.endMs, positionMs.value),
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

/** 短纯文本和未高亮的第二行无需读取高频时钟；溢出行仍按原有时间轴滚动。 */
const positionMs = computed(() =>
  overflowDistance.value > 0 || (props.primary && renderWords.value.length > 0)
    ? lyricsPositionMs.value
    : props.line.startMs,
)

/** 逐字高亮状态；非主行不参与，避免翻译行被当前词的渐变染色。 */
const highlight = computed<WordHighlightState>((previous) => {
  const next = props.primary
    ? resolveWordHighlightState(renderWords.value, positionMs.value)
    : { completedCount: 0, activeIndex: -1 }
  // 完整计算后再复用旧对象，让 Vue 在词内进度变化时跳过整行依赖更新。
  return previous?.completedCount === next.completedCount &&
    previous.activeIndex === next.activeIndex
    ? previous
    : next
})

/** 返回稳定类名，避免为非活动词创建样式对象。 */
function wordStateClass(index: number) {
  if (index < highlight.value.completedCount) return 'lyric-word-completed'
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
        <span
          ref="textMeasure"
          v-memo="[renderWords, text, highlight]"
          class="inline-block w-max align-top"
        >
          <template v-if="renderWords.length > 0">
            <template v-for="(word, index) in renderWords" :key="`${word.startMs}-${index}`">
              <LyricActiveWordElement v-if="index === highlight.activeIndex" :word="word" />
              <span v-else class="lyric-word" :class="wordStateClass(index)">{{ word.text }}</span>
            </template>
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
  /* 与当前词保持同一绘制方式，避免切换到渐变时字形边缘出现明暗跳变。 */
  -webkit-text-fill-color: transparent;
}

.lyric-word-pending {
  background-image: linear-gradient(var(--lyric-unplayed-color), var(--lyric-unplayed-color));
}

.lyric-word-completed {
  background-image: linear-gradient(var(--lyric-played-color), var(--lyric-played-color));
}
</style>
