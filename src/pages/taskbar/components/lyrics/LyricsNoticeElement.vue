<script setup lang="ts">
import type { CSSProperties, DeepReadonly } from 'vue'
import { computed } from 'vue'

import { resolveTaskbarLyricsAppearance } from '@/features/lyrics/appearance'
import type { TaskbarLyricsAlignment, TaskbarLyricsSettings } from '@/features/settings/lyrics'

const alignmentClasses: Record<TaskbarLyricsAlignment, string> = {
  left: 'text-left',
  center: 'text-center',
  right: 'text-right',
}

const props = defineProps<{
  text: string
  settings: DeepReadonly<TaskbarLyricsSettings>
  themeColor: string
}>()

/** 复用歌词字体、颜色和对齐设置，但不创建任何播放时间轴。 */
const noticeStyle = computed<CSSProperties>(() => {
  const appearance = resolveTaskbarLyricsAppearance(props.settings, props.themeColor)
  const lineHeight = props.settings.fontSize === 14 ? 17.5 : props.settings.fontSize + 2
  return {
    color: appearance.playedColor,
    fontFamily: appearance.fontFamily,
    fontSize: `${props.settings.fontSize}px`,
    lineHeight: `${lineHeight}px`,
  }
})
</script>

<template>
  <div
    class="flex h-full min-w-0 flex-1 items-center overflow-hidden font-medium"
    :class="alignmentClasses[settings.alignment]"
    :style="noticeStyle"
    :aria-label="text"
  >
    <span class="w-full truncate">{{ text }}</span>
  </div>
</template>
