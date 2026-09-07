<script setup lang="ts">
import { useElementSize } from '@vueuse/core'
import { motion, useReducedMotion } from 'motion-v'
import { computed, useTemplateRef } from 'vue'

import type { TaskbarTrackInfoScrolling } from '@/features/settings/track-info'

const LOOP_GAP_PX = 24

const props = defineProps<{
  text: string
  scrolling: TaskbarTrackInfoScrolling
}>()

const viewport = useTemplateRef<HTMLElement>('viewport')
const titleMeasure = useTemplateRef<HTMLElement>('titleMeasure')
const { width: viewportWidth } = useElementSize(viewport)
const { width: titleWidth } = useElementSize(titleMeasure)
const prefersReducedMotion = useReducedMotion()

/** 计算文字末尾完整进入显示区域时需要移动的距离。 */
const overflowDistance = computed(() => Math.max(0, titleWidth.value - viewportWidth.value))

/** 只有配置开启、文字确实溢出且系统允许动效时才创建滚动动画。 */
const shouldScroll = computed(
  () => props.scrolling.enabled && !prefersReducedMotion.value && overflowDistance.value > 0,
)

/** 无缝循环需要跨过副本间距，其余方式只移动实际溢出距离。 */
const animationDistance = computed(() =>
  props.scrolling.mode === 'loop' ? titleWidth.value + LOOP_GAP_PX : overflowDistance.value,
)

/** 将像素每秒换算为单程动画时长。 */
const animationDuration = computed(() => animationDistance.value / props.scrolling.speed)

/** 配置或尺寸变化时重建动画，确保新速度和新边界立即生效。 */
const animationKey = computed(
  () =>
    `${props.text}:${props.scrolling.mode}:${props.scrolling.speed}:${viewportWidth.value}:${titleWidth.value}`,
)

const animationTarget = computed(() => ({ x: [0, -animationDistance.value] }))
const animationTransition = computed(() => ({
  type: 'tween' as const,
  duration: animationDuration.value,
  ease: 'linear' as const,
  repeat: Number.POSITIVE_INFINITY,
  repeatType: props.scrolling.mode === 'alternate' ? ('reverse' as const) : ('loop' as const),
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

    <motion.div
      v-if="shouldScroll"
      :key="animationKey"
      class="flex w-max will-change-transform"
      :initial="{ x: 0 }"
      :animate="animationTarget"
      :transition="animationTransition"
      aria-hidden="true"
    >
      <span class="whitespace-nowrap">{{ text }}</span>
      <span v-if="scrolling.mode === 'loop'" class="ml-6 whitespace-nowrap" aria-hidden="true">
        {{ text }}
      </span>
    </motion.div>
    <span v-else class="block truncate" aria-hidden="true">{{ text }}</span>
  </div>
</template>
