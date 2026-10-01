<script setup lang="ts">
import type {
  TaskbarProgressPosition,
  TaskbarProgressStyle,
} from '@/features/settings/progress-style'
import { useTaskbarPlaybackClock } from '@/features/taskbar/playback-clock'

const props = defineProps<{ mode: TaskbarProgressStyle; position: TaskbarProgressPosition }>()
const { t } = useI18n({ useScope: 'global' })
const { progress } = useTaskbarPlaybackClock()

/** 进度仅改变合成变换，样式对象在本组件内更新，不触发任务栏布局渲染。 */
const progressStyle = computed(() => ({
  width: '100%',
  transform: `scaleX(${progress.value / 100})`,
  transformOrigin: 'left center',
  background:
    props.mode === 'vertical-gradient'
      ? 'linear-gradient(to right, transparent 0%, color-mix(in srgb, var(--taskbar-progress-color) 40%, var(--taskbar-background)) 100%)'
      : undefined,
}))
</script>

<template>
  <div
    class="pointer-events-none absolute inset-0"
    role="progressbar"
    :aria-label="t('media.progress')"
    aria-valuemin="0"
    aria-valuemax="100"
    :aria-valuenow="Math.round(progress)"
  >
    <div
      v-if="mode === 'bottom'"
      class="absolute left-0 z-0 h-0.5 bg-(--taskbar-progress-color)"
      :class="position === 'top' ? 'top-0' : 'bottom-0'"
      :style="progressStyle"
    />
    <div
      v-else-if="mode === 'vertical-gradient'"
      class="absolute inset-y-0 left-0 z-0"
      :style="progressStyle"
    />
  </div>
</template>
