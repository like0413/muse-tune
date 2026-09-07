<script setup lang="ts">
import { computed } from 'vue'

import { getMediaPlayerPresentation } from '@/features/media/players'
import type { MediaPlayer } from '@/features/media/types'

const props = defineProps<{
  player: MediaPlayer
  iconDataUrl: string | null
}>()

const presentation = computed(() => getMediaPlayerPresentation(props.player))
</script>

<template>
  <span
    class="absolute -right-0.5 -bottom-0.5 z-10 grid size-3.5 place-items-center overflow-hidden rounded-lg text-[8px] leading-none font-bold shadow-sm ring-1 ring-black/30 dark:ring-white/30"
    :class="iconDataUrl ? 'bg-white' : presentation.fallbackClass"
    :title="presentation.label"
    aria-hidden="true"
  >
    <img v-if="iconDataUrl" class="size-full object-contain" :src="iconDataUrl" alt="" />
    <span v-else>{{ presentation.fallbackText }}</span>
  </span>
</template>
