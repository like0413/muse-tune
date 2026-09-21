<script setup lang="ts">
import { useElementSize } from '@vueuse/core'

import { useReducedMotionPreference } from '@/features/motion/useReducedMotionPreference'
import type { TaskbarTrackInfoScrolling } from '@/features/settings/track-info'

const props = defineProps<{
  text: string
  scrolling: TaskbarTrackInfoScrolling
  /** 所在内容层是否可见；不可见时停止滚动，避免无限动画持续驱动合成器出帧。 */
  active: boolean
}>()

const viewport = useTemplateRef<HTMLElement>('viewport')
const titleMeasure = useTemplateRef<HTMLElement>('titleMeasure')
const { width: viewportWidth } = useElementSize(viewport)
const { width: titleWidth } = useElementSize(titleMeasure)
const prefersReducedMotion = useReducedMotionPreference()

const overflowDistance = computed(() => Math.max(0, titleWidth.value - viewportWidth.value))

/** 只有所在层可见、配置开启、文字确实溢出且系统允许动效时才创建滚动动画。 */
const shouldScroll = computed(
  () =>
    props.active &&
    props.scrolling.enabled &&
    !prefersReducedMotion.value &&
    overflowDistance.value > 0,
)

/** 循环滚动跨过一个视口宽度，前一份离开时副本恰好进入。 */
const animationDistance = computed(() =>
  props.scrolling.mode === 'loop' ? titleWidth.value + viewportWidth.value : overflowDistance.value,
)

const loopSpacerStyle = computed(() => ({ width: `${viewportWidth.value}px` }))

/** 将像素每秒换算为单程动画时长。 */
const animationDuration = computed(() => animationDistance.value / props.scrolling.speed)

/** 配置或尺寸变化时重建动画，确保新速度和新边界立即生效。 */
const animationKey = computed(
  () =>
    `${props.text}:${props.scrolling.mode}:${props.scrolling.speed}:${viewportWidth.value}:${titleWidth.value}`,
)

const animationStyle = computed(() => ({
  '--scroll-distance': `-${animationDistance.value}px`,
  '--scroll-duration': `${animationDuration.value}s`,
}))
</script>

<template>
  <div ref="viewport" class="relative w-full overflow-hidden">
    <span class="sr-only">{{ text }}</span>
    <span
      ref="titleMeasure"
      class="pointer-events-none invisible absolute top-0 left-0 w-max whitespace-nowrap"
      aria-hidden="true"
    >
      {{ text }}
    </span>

    <div
      v-if="shouldScroll"
      :key="animationKey"
      class="track-title-scroll flex w-max"
      :class="{ 'track-title-scroll-alternate': scrolling.mode === 'alternate' }"
      :style="animationStyle"
      aria-hidden="true"
    >
      <span class="whitespace-nowrap">{{ text }}</span>
      <span v-if="scrolling.mode === 'loop'" class="shrink-0" :style="loopSpacerStyle" />
      <span v-if="scrolling.mode === 'loop'" class="whitespace-nowrap" aria-hidden="true">
        {{ text }}
      </span>
    </div>
    <span v-else class="block truncate" aria-hidden="true">{{ text }}</span>
  </div>
</template>

<style scoped>
.track-title-scroll {
  animation: track-title-scroll var(--scroll-duration) linear infinite;
}

.track-title-scroll-alternate {
  animation-direction: alternate;
}

@keyframes track-title-scroll {
  from {
    transform: translate3d(0, 0, 0);
  }

  to {
    transform: translate3d(var(--scroll-distance), 0, 0);
  }
}
</style>
