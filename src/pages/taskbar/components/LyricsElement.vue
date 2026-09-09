<script setup lang="ts">
import type { CSSProperties, DeepReadonly } from 'vue'

import type { LyricWord, LyricsSnapshot } from '@/features/lyrics/types'
import type { TaskbarLyricsSettings } from '@/features/settings/lyrics'

const alignmentClasses: Record<TaskbarLyricsSettings['alignment'], string> = {
  left: 'text-left',
  center: 'text-center',
  right: 'text-right',
}

const props = defineProps<{
  lyrics: DeepReadonly<LyricsSnapshot>
  positionMs: number
  settings: DeepReadonly<TaskbarLyricsSettings>
}>()

interface DisplayWord extends LyricWord {
  state: 'pending' | 'active' | 'completed'
  style?: CSSProperties
}

/** 使用二分查找定位当前行，避免每个动画帧线性扫描整首歌词。 */
const currentLineIndex = computed(() => {
  const lines = props.lyrics.lines
  if (lines.length === 0) return -1
  let left = 0
  let right = lines.length - 1
  let matched = 0
  while (left <= right) {
    const middle = Math.floor((left + right) / 2)
    if (lines[middle]!.startMs <= props.positionMs) {
      matched = middle
      left = middle + 1
    } else {
      right = middle - 1
    }
  }
  return matched
})

const currentLine = computed(() => props.lyrics.lines[currentLineIndex.value] ?? null)
const secondaryText = computed(() => {
  const line = currentLine.value
  if (!line) return ''
  return line.translation || props.lyrics.lines[currentLineIndex.value + 1]?.text || ''
})
const alignmentClass = computed(() => alignmentClasses[props.settings.alignment])
const displayWords = computed<DisplayWord[]>(() => {
  if (!props.settings.wordHighlight) return []
  const words = currentLine.value?.words ?? []
  return words.map((word) => {
    if (props.positionMs >= word.endMs) return { ...word, state: 'completed' }
    if (props.positionMs < word.startMs) return { ...word, state: 'pending' }
    const duration = Math.max(1, word.endMs - word.startMs)
    const progress = Math.min(
      100,
      Math.max(0, ((props.positionMs - word.startMs) / duration) * 100),
    )
    return {
      ...word,
      state: 'active',
      style: { '--lyric-word-progress': `${progress}%` } as CSSProperties,
    }
  })
})
</script>

<template>
  <div
    class="lyric-display flex h-full min-w-0 flex-1 flex-col justify-center"
    :class="alignmentClass"
    aria-label="当前歌词"
  >
    <div class="lyric-primary truncate text-sm font-medium">
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
      <span v-else>{{ currentLine?.text }}</span>
    </div>
    <div v-if="settings.lineMode === 'double'" class="text-foreground/70 truncate text-xs">
      {{ secondaryText }}
    </div>
  </div>
</template>

<style scoped>
.lyric-display {
  line-height: 1.25;
}

.lyric-word-pending {
  color: color-mix(in srgb, currentColor 42%, transparent);
}

.lyric-word-completed {
  color: inherit;
}

.lyric-word-active {
  color: transparent;
  background-image: linear-gradient(
    to right,
    var(--taskbar-foreground) var(--lyric-word-progress),
    color-mix(in srgb, var(--taskbar-foreground) 42%, transparent) var(--lyric-word-progress)
  );
  background-clip: text;
}
</style>
