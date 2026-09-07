<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { onMounted, onUnmounted, shallowRef } from 'vue'

import {
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
  DEFAULT_TASKBAR_TRACK_INFO_SCROLLING,
  getTaskbarTrackInfoAlignment,
  getTaskbarTrackInfoScrolling,
  listenTaskbarTrackInfoAlignmentChange,
  listenTaskbarTrackInfoScrollingChange,
  type TaskbarTrackInfoAlignment,
  type TaskbarTrackInfoScrolling,
} from '@/features/settings/track-info'

import ScrollingTrackTitle from './track-info/ScrollingTrackTitle.vue'

const alignment = shallowRef<TaskbarTrackInfoAlignment>(DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT)
const scrolling = shallowRef<TaskbarTrackInfoScrolling>({
  ...DEFAULT_TASKBAR_TRACK_INFO_SCROLLING,
})
let unlistenAlignmentChange: UnlistenFn | undefined
let unlistenScrollingChange: UnlistenFn | undefined

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

/** 恢复歌名滚动配置，并接收设置窗口的实时更新与速度预览。 */
async function initializeScrolling() {
  try {
    unlistenScrollingChange = await listenTaskbarTrackInfoScrollingChange((value) => {
      scrolling.value = value
    })
    scrolling.value = await getTaskbarTrackInfoScrolling()
  } catch (error) {
    console.error('初始化歌名滚动配置失败', error)
  }
}

onMounted(initializeAlignment)
onMounted(initializeScrolling)
onUnmounted(() => {
  unlistenAlignmentChange?.()
  unlistenScrollingChange?.()
})
</script>

<template>
  <div
    class="flex min-w-7 flex-1 flex-col justify-center leading-tight"
    :class="alignment === 'right' ? 'items-end text-right' : 'items-start text-left'"
  >
    <ScrollingTrackTitle class="text-sm font-medium" text="洛阳纸" :scrolling="scrolling" />
    <span class="text-muted-foreground max-w-full truncate text-xs">许嵩</span>
  </div>
</template>
