<script setup lang="ts">
import { createColor } from 'colorthief'
import type { CSSProperties } from 'vue'

import type { TaskbarCoverShape } from '@/features/settings/cover'
import { useTaskbarPlaybackClock } from '@/features/taskbar/playback-clock'

const props = defineProps<{
  color: string
  shape: TaskbarCoverShape
}>()

const { progress } = useTaskbarPlaybackClock()

/**
 * 描边中心线位于 32px 封面边界外 0.5px：2px 描边向内覆盖封面边缘 0.5px，
 * 消除抗锯齿产生的视觉细缝；三种路径均从顶部中心开始并顺时针闭合。
 */
const SHAPE_PATHS: Record<TaskbarCoverShape, string> = {
  square: 'M 16 -0.5 H 32.5 V 32.5 H -0.5 V -0.5 H 16 Z',
  rounded:
    'M 16 -0.5 H 24 A 8.5 8.5 0 0 1 32.5 8 V 24 A 8.5 8.5 0 0 1 24 32.5 H 8 A 8.5 8.5 0 0 1 -0.5 24 V 8 A 8.5 8.5 0 0 1 8 -0.5 H 16 Z',
  circle: 'M 16 -0.5 A 16.5 16.5 0 1 1 16 32.5 A 16.5 16.5 0 1 1 16 -0.5 Z',
}

/** 将外部播放进度限制到 SVG 路径使用的 0–100 范围。 */
const progressStyle = computed<CSSProperties>(() => ({
  strokeDashoffset: 100 - Math.min(100, Math.max(0, progress.value)),
}))

/** 在 OKLCH 色环上旋转 180°，保留主题色原有的感知明度与色度。 */
function getComplementaryColor(hex: string): string {
  if (!/^#[\da-f]{6}$/i.test(hex)) return hex

  const value = Number.parseInt(hex.slice(1), 16)
  const { l, c, h } = createColor(
    (value >> 16) & 0xff,
    (value >> 8) & 0xff,
    value & 0xff,
    0,
  ).oklch()
  if (!Number.isFinite(h)) return hex
  return `oklch(${l} ${c} ${(h + 180) % 360})`
}

const ringStyle = computed<CSSProperties>(() => ({ color: getComplementaryColor(props.color) }))
const ringPath = computed(() => SHAPE_PATHS[props.shape])
</script>

<template>
  <svg
    class="pointer-events-none absolute inset-0 z-10 size-full overflow-visible"
    viewBox="0 0 32 32"
    :style="ringStyle"
    aria-hidden="true"
  >
    <path
      class="cover-progress-track"
      :d="ringPath"
      pathLength="100"
      fill="none"
      stroke-width="3"
    />
    <path
      class="cover-progress-value"
      :d="ringPath"
      pathLength="100"
      fill="none"
      stroke-width="3"
      stroke-linecap="round"
      stroke-dasharray="100"
      :style="progressStyle"
    />
  </svg>
</template>

<style scoped>
.cover-progress-track {
  stroke: #dadada;
  opacity: 0.25;
}

.cover-progress-value {
  stroke: currentColor;
}
</style>
