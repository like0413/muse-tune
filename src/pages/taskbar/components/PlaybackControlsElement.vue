<script setup lang="ts">
import { Pause, Play, SkipBack, SkipForward } from '@lucide/vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { onMounted, onUnmounted, shallowRef } from 'vue'

import { Button } from '@/components/ui/button'
import type { MediaControlAction, MediaSessionSnapshot } from '@/features/media/types'
import {
  DEFAULT_TASKBAR_CONTROLS_VISIBILITY,
  getTaskbarControlsVisibility,
  listenTaskbarControlsVisibilityChange,
  type TaskbarControlsVisibility,
} from '@/features/settings/controls'

const props = defineProps<{ session: MediaSessionSnapshot | null; pending: boolean }>()
const emit = defineEmits<{ control: [action: MediaControlAction] }>()
const visibility = shallowRef<TaskbarControlsVisibility>({ ...DEFAULT_TASKBAR_CONTROLS_VISIBILITY })
let unlistenVisibilityChange: UnlistenFn | undefined

const isPlaying = computed(() => props.session?.playback.status === 'playing')
const canTogglePlayback = computed(() => {
  const playback = props.session?.playback
  if (!playback) return false
  return (
    playback.controls.canTogglePlayPause ||
    (isPlaying.value ? playback.controls.canPause : playback.controls.canPlay)
  )
})

/** 恢复按钮显示配置，并接收设置窗口的实时更新。 */
async function initializeVisibility() {
  try {
    unlistenVisibilityChange = await listenTaskbarControlsVisibilityChange((value) => {
      visibility.value = value
    })
    visibility.value = await getTaskbarControlsVisibility()
  } catch (error) {
    console.error('初始化控制按钮配置失败', error)
  }
}

onMounted(initializeVisibility)
onUnmounted(() => unlistenVisibilityChange?.())
</script>

<template>
  <div v-if="visibility.visible" class="flex shrink-0" aria-label="播放控制">
    <Button
      v-if="visibility.previous"
      variant="ghost"
      size="icon-sm"
      class="taskbar-control"
      type="button"
      title="上一曲"
      aria-label="上一曲"
      :disabled="pending || !session?.playback.controls.canSkipPrevious"
      @click="emit('control', 'skip_previous')"
    >
      <SkipBack class="fill-current" data-icon="inline-start" />
    </Button>
    <Button
      v-if="visibility.playPause"
      variant="ghost"
      size="icon-sm"
      class="taskbar-control"
      type="button"
      title="播放或暂停"
      aria-label="播放或暂停"
      :disabled="pending || !canTogglePlayback"
      @click="emit('control', 'toggle_play_pause')"
    >
      <Pause v-if="isPlaying" class="fill-current" data-icon="inline-start" />
      <Play v-else class="fill-current" data-icon="inline-start" />
    </Button>
    <Button
      v-if="visibility.next"
      variant="ghost"
      size="icon-sm"
      class="taskbar-control"
      type="button"
      title="下一曲"
      aria-label="下一曲"
      :disabled="pending || !session?.playback.controls.canSkipNext"
      @click="emit('control', 'skip_next')"
    >
      <SkipForward class="fill-current" data-icon="inline-start" />
    </Button>
  </div>
</template>

<style scoped>
/* Hover 背景以当前实际前景色生成，透明 bar 也能保持对比度。 */
.taskbar-control:hover {
  color: inherit;
  background-color: color-mix(in srgb, currentColor 20%, transparent);
}
</style>
