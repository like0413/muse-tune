<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { onMounted, onUnmounted, shallowRef } from 'vue'

import {
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
  getTaskbarTrackInfoAlignment,
  listenTaskbarTrackInfoAlignmentChange,
  type TaskbarTrackInfoAlignment,
} from '@/features/settings/track-info'

const alignment = shallowRef<TaskbarTrackInfoAlignment>(DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT)
let unlistenAlignmentChange: UnlistenFn | undefined

/** 恢复歌曲信息对齐方式，并接收设置窗口的实时更新。 */
async function initializeAlignment() {
  try {
    unlistenAlignmentChange = await listenTaskbarTrackInfoAlignmentChange((value) => {
      alignment.value = value
    })
    alignment.value = await getTaskbarTrackInfoAlignment()
  } catch (error) {
    console.error('初始化歌曲信息对齐方式失败', error)
  }
}

onMounted(initializeAlignment)
onUnmounted(() => unlistenAlignmentChange?.())
</script>

<template>
  <div
    class="flex min-w-7 flex-1 flex-col justify-center leading-tight"
    :class="alignment === 'right' ? 'items-end text-right' : 'items-start text-left'"
  >
    <span class="max-w-full truncate text-sm font-medium">洛阳纸</span>
    <span class="text-muted-foreground max-w-full truncate text-xs">许嵩</span>
  </div>
</template>
