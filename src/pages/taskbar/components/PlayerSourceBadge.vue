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
    class="absolute right-0 bottom-0 z-10 grid size-2.5 place-items-center overflow-hidden rounded-sm text-[6px] leading-none font-bold"
    :class="iconDataUrl ? '' : presentation.fallbackClass"
    :title="presentation.label"
    aria-hidden="true"
  >
    <img v-if="iconDataUrl" class="size-full object-contain" :src="iconDataUrl" alt="" />
    <span v-else>{{ presentation.fallbackText }}</span>
  </span>
</template>
