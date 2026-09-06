<script setup lang="ts">
import { Pause, Play, SkipBack, SkipForward } from '@lucide/vue'
import { shallowRef } from 'vue'

import { Button } from '@/components/ui/button'

const isPlaying = shallowRef(true)

/** 切换当前播放状态。 */
function togglePlayback() {
  isPlaying.value = !isPlaying.value
}
</script>

<template>
  <div class="flex shrink-0 gap-1" aria-label="播放控制">
    <Button variant="secondary" size="icon-sm" type="button" title="上一曲" aria-label="上一曲">
      <SkipBack data-icon="inline-start" />
    </Button>
    <Button
      variant="secondary"
      size="icon-sm"
      type="button"
      title="播放或暂停"
      aria-label="播放或暂停"
      @click="togglePlayback"
    >
      <Pause v-if="isPlaying" data-icon="inline-start" />
      <Play v-else data-icon="inline-start" />
    </Button>
    <Button variant="secondary" size="icon-sm" type="button" title="下一曲" aria-label="下一曲">
      <SkipForward data-icon="inline-start" />
    </Button>
  </div>
</template>
