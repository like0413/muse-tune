<script setup lang="ts">
import type { CSSProperties, DeepReadonly } from 'vue'

import { resolveTaskbarLyricsAppearance } from '@/features/lyrics/appearance'
import {
  resolveCurrentLineIndex,
  resolveLayoutMetrics,
  resolveTransitionDurationMs,
  resolveTransitionLeadMs,
  selectSecondaryContent,
  type SecondaryContent,
} from '@/features/lyrics/line-window'
import type { LyricLine, LyricsSnapshot } from '@/features/lyrics/types'
import { useReducedMotionPreference } from '@/features/motion/useReducedMotionPreference'
import type { TaskbarLyricsSettings } from '@/features/settings/lyrics'
import {
  provideTaskbarPlaybackClock,
  useTaskbarPlaybackClock,
} from '@/features/taskbar/playback-clock'

import LyricLineElement from './lyrics/LyricLineElement.vue'

const props = defineProps<{
  lyrics: DeepReadonly<LyricsSnapshot>
  settings: DeepReadonly<TaskbarLyricsSettings>
  themeColor: string
  active: boolean
}>()

const clock = useTaskbarPlaybackClock()
/** 被普通层或音量层遮住时冻结歌词时钟；再次可见时直接追平当前进度。 */
const lyricsPositionMs = computed<number>((previous) =>
  props.active || previous === undefined ? clock.lyricsPositionMs.value : previous,
)
provideTaskbarPlaybackClock({ progress: clock.progress, lyricsPositionMs })
const reducedMotion = useReducedMotionPreference()
/** 减少动态效果时彻底跳过 Vue 过渡，避免零时长透明度状态产生闪烁。 */
const lyricsAnimationEnabled = computed(
  () => props.settings.animation !== 'none' && !reducedMotion.value,
)
const resuming = shallowRef(false)
/** 恢复显示时直接切到当前行，避免把冻结期间的旧句子做一次可见过渡。 */
watch(
  () => props.active,
  (active) => {
    if (!active) return
    resuming.value = true
    void nextTick(() => (resuming.value = false))
  },
)
const transitionsEnabled = computed(
  () => lyricsAnimationEnabled.value && props.active && !resuming.value,
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

/** 当前动画的切换时长；交叉淡化比位移动画更慢，见 `resolveTransitionDurationMs`。 */
const transitionDurationMs = computed(() => resolveTransitionDurationMs(props.settings.animation))

// Vue 3.4+ 的 computed 会抑制等值通知；时钟每帧推进，行数据只在索引改变时更新。
const activeLineIndex = computed(() =>
  resolveCurrentLineIndex(
    props.lyrics.lines,
    lyricsPositionMs.value,
    resolveTransitionLeadMs(
      lyricsAnimationEnabled.value,
      props.settings.animationPreRoll,
      transitionDurationMs.value,
    ),
  ),
)

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

const layoutMetrics = computed(() =>
  resolveLayoutMetrics(props.settings.fontSize, hasSecondaryLine.value),
)

/**
 * 行标识。
 *
 * `up` 的滚动感来自"同一行从第二槽位升到第一槽位"，两个槽位必须共用一个 key 才会被复用。
 * `fade` 要求任何一行都不位移，于是把槽位并进 key：切换时两个槽位各自渐隐渐显，元素永不复用，
 * 也就不会出现跨槽位的补间。
 */
function lineKey(slot: 'primary' | 'secondary', content: string): string {
  const trackKey = props.lyrics.trackKey ?? 'unknown'
  return props.settings.animation === 'fade'
    ? `${trackKey}:${slot}:${content}`
    : `${trackKey}:${content}`
}

/** 生成最多两行稳定标识的数据；key 的取舍见 `lineKey`。 */
const displayLines = computed<DisplayLine[]>(() => {
  const index = activeLineIndex.value
  const current = props.lyrics.lines[index]
  if (!current) return []
  const lines: DisplayLine[] = [
    {
      key: lineKey('primary', `${index}:original`),
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
      key: lineKey(
        'secondary',
        secondary.kind === 'translation' ? `${index}:translation` : `${index + 1}:original`,
      ),
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
    '--lyric-duration-ms': `${transitionDurationMs.value}ms`,
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
    :css="transitionsEnabled"
    :style="displayStyle"
  >
    <LyricLineElement
      v-for="line in displayLines"
      :key="line.key"
      :line="line.line"
      :text="line.text"
      :primary="line.primary"
      :word-highlight="settings.wordHighlight"
      :animated="transitionsEnabled"
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

/*
 * 原地渐隐渐显：只做透明度，任何一行都不平移。
 *
 * key 里带了槽位（见 `lineKey`），切换时两个槽位都是"旧行离开 + 新行进入"，
 * 不存在跨槽位复用的元素，因此不会产生位移补间，也不需要 move 类。
 */
.lyrics-fade-enter-active,
.lyrics-fade-leave-active {
  transition: opacity var(--lyric-duration-ms) ease;
}

/* 离开行固定在原槽位，把位置让给淡入的新行；两者叠在同一槽位完成交叉淡变。 */
.lyrics-fade-leave-active {
  position: absolute;
  top: var(--lyric-row-top);
  width: 100%;
}

.lyrics-fade-enter-from,
.lyrics-fade-leave-to {
  opacity: 0;
}
</style>
