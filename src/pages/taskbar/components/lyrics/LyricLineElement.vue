<script setup lang="ts">
import { useElementSize } from '@vueuse/core'
import type { CSSProperties, DeepReadonly } from 'vue'

import type { LyricLine, LyricWord } from '@/features/lyrics/types'
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

interface DisplayWord extends LyricWord {
  state: 'pending' | 'active' | 'completed'
  style?: CSSProperties
}

const viewport = useTemplateRef<HTMLElement>('viewport')
const textMeasure = useTemplateRef<HTMLElement>('textMeasure')
const { width: viewportWidth } = useElementSize(viewport)
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
  fontSize: `${props.fontSize}px`,
  height: `${props.lineHeight}px`,
  lineHeight: `${props.lineHeight}px`,
}))
const trackClass = computed(() => [
  alignmentClasses[props.alignment],
  overflowDistance.value > 0 ? 'w-max will-change-transform' : 'w-full',
])

/** 生成仅在未播放色和已播放色之间变化的逐字渐变。 */
const displayWords = computed<DisplayWord[]>(() => {
  if (!props.primary || !props.wordHighlight || props.text !== props.line.text) return []
  return props.line.words.map((word) => {
    if (props.positionMs >= word.endMs) return { ...word, state: 'completed' }
    if (props.positionMs <= word.startMs) return { ...word, state: 'pending' }

    const duration = Math.max(1, word.endMs - word.startMs)
    const progress = Math.min(
      100,
      Math.max(0, ((props.positionMs - word.startMs) / duration) * 100),
    )
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
      ...word,
      state: 'active',
      style: {
        '--lyric-word-gradient-start': `${start}%`,
        '--lyric-word-gradient-center': `${progress}%`,
        '--lyric-word-gradient-end': `${end}%`,
        '--lyric-word-gradient-start-color': `color-mix(in srgb, var(--lyric-played-color) ${startStrength}%, var(--lyric-unplayed-color))`,
        '--lyric-word-gradient-center-color': `color-mix(in srgb, var(--lyric-played-color) ${centerStrength}%, var(--lyric-unplayed-color))`,
        '--lyric-word-gradient-end-color': `color-mix(in srgb, var(--lyric-played-color) ${endStrength}%, var(--lyric-unplayed-color))`,
      } as CSSProperties,
    }
  })
})
</script>

<template>
  <div
    class="lyric-row w-full min-w-0 overflow-hidden"
    :class="[primary ? 'font-medium' : 'font-normal', { 'lyric-row-animated': animated }]"
    :style="rowStyle"
    aria-hidden="true"
  >
    <div ref="viewport" class="relative w-full overflow-hidden">
      <span
        ref="textMeasure"
        class="pointer-events-none invisible absolute top-0 left-0 w-max whitespace-pre"
      >
        {{ text }}
      </span>
      <div class="lyric-line-track whitespace-pre" :class="trackClass" :style="trackStyle">
        <template v-if="displayWords.length > 0">
          <span
            v-for="(word, index) in displayWords"
            :key="`${word.startMs}-${index}`"
            class="lyric-word"
            :class="`lyric-word-${word.state}`"
            :style="word.style"
            >{{ word.text }}</span
          >
        </template>
        <span v-else class="lyric-plain-text">{{ text }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.lyric-row {
  display: flex;
  align-items: center;
}

.lyric-row-animated {
  transition:
    height 320ms cubic-bezier(0.22, 1, 0.36, 1),
    line-height 320ms cubic-bezier(0.22, 1, 0.36, 1),
    font-size 320ms cubic-bezier(0.22, 1, 0.36, 1);
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

@media (prefers-reduced-motion: reduce) {
  .lyric-row-animated {
    transition-duration: 0s;
  }
}
</style>
