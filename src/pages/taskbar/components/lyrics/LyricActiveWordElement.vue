<script setup lang="ts">
import type { CSSProperties, DeepReadonly } from 'vue'

import type { LyricWord } from '@/features/lyrics/types'
import { resolveWordGradient } from '@/features/lyrics/word-highlight'
import { useTaskbarPlaybackClock } from '@/features/taskbar/playback-clock'

const props = defineProps<{ word: DeepReadonly<LyricWord> }>()
const { lyricsPositionMs } = useTaskbarPlaybackClock()

/** 高频渐变只更新当前词；整行词列表仅在播放边界变化时更新。 */
const gradientStyle = computed<CSSProperties>(() => {
  const gradient = resolveWordGradient(lyricsPositionMs.value, props.word)
  return {
    '--lyric-word-gradient-start': `${gradient.progressStart}%`,
    '--lyric-word-gradient-center': `${gradient.progressCenter}%`,
    '--lyric-word-gradient-end': `${gradient.progressEnd}%`,
    '--lyric-word-gradient-start-color': `color-mix(in srgb, var(--lyric-played-color) ${gradient.startStrength}%, var(--lyric-unplayed-color))`,
    '--lyric-word-gradient-center-color': `color-mix(in srgb, var(--lyric-played-color) ${gradient.centerStrength}%, var(--lyric-unplayed-color))`,
    '--lyric-word-gradient-end-color': `color-mix(in srgb, var(--lyric-played-color) ${gradient.endStrength}%, var(--lyric-unplayed-color))`,
  } as CSSProperties
})
</script>

<template>
  <span class="lyric-word-active" :style="gradientStyle">{{ word.text }}</span>
</template>

<style scoped>
.lyric-word-active {
  white-space: pre;
  background-clip: text;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
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
