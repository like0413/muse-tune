<script setup lang="ts">
import type { DeepReadonly } from 'vue'

import type { MediaSessionSnapshot } from '@/features/media/types'
import type { TaskbarCoverAppearance } from '@/features/settings/cover'

import CoverProgressRing from './CoverProgressRing.vue'
import PlayerSourceBadge from './PlayerSourceBadge.vue'

const props = defineProps<{
  session: MediaSessionSnapshot | null
  appearance: DeepReadonly<TaskbarCoverAppearance>
  thumbnailDataUrl: string | null
  progress: number
  progressColor: string
  showProgressRing: boolean
}>()

/** 依据保存的形状与播放状态生成封面图层类名。 */
const shapeClass = computed(() => ({
  'rounded-none': props.appearance.shape === 'square',
  'rounded-md': props.appearance.shape === 'rounded',
  'rounded-full': props.appearance.shape === 'circle',
  'cover-rotating': props.appearance.shape === 'circle' && props.appearance.rotateWhenPlaying,
  'cover-rotation-paused':
    props.appearance.shape === 'circle' &&
    props.appearance.rotateWhenPlaying &&
    props.session?.playback.status !== 'playing',
}))
</script>

<template>
  <div class="relative size-8 shrink-0" aria-hidden="true">
    <div
      class="bg-primary text-primary-foreground grid size-full place-items-center overflow-hidden text-base font-medium"
      :class="shapeClass"
    >
      <img v-if="thumbnailDataUrl" class="size-full object-cover" :src="thumbnailDataUrl" alt="" />
      <span v-else>♪</span>
    </div>
    <CoverProgressRing
      v-if="showProgressRing"
      :progress="progress"
      :color="progressColor"
      :shape="appearance.shape"
    />
    <PlayerSourceBadge
      v-if="appearance.showPlayerSource && session"
      :player="session.player"
      :icon-data-url="session.sourceIconDataUrl ?? null"
    />
  </div>
</template>

<style scoped>
/* 旋转只作用于封面图层，右下角播放器来源徽标保持静止。 */
.cover-rotating {
  animation: cover-rotation 12s linear infinite;
}

.cover-rotation-paused {
  animation-play-state: paused;
}

@keyframes cover-rotation {
  to {
    transform: rotate(1turn);
  }
}
</style>
