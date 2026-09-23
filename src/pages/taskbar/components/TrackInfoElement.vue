<script setup lang="ts">
import { useEventState } from '@/features/ipc/useEventState'
import {
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
  DEFAULT_TASKBAR_TRACK_INFO_SCROLLING,
  getTaskbarTrackInfoAlignment,
  getTaskbarTrackInfoScrolling,
  getTaskbarTrackInfoVisible,
  listenTaskbarTrackInfoAlignmentChange,
  listenTaskbarTrackInfoScrollingChange,
  listenTaskbarTrackInfoVisibleChange,
} from '@/features/settings/track-info'

import ScrollingTrackTitle from './ScrollingTrackTitle.vue'

const props = defineProps<{
  title: string
  artist: string
  /** 所在内容层是否可见；仅在可见时才继续运行标题滚动动画。 */
  active: boolean
}>()
const alignment = useEventState(
  {
    read: getTaskbarTrackInfoAlignment,
    subscribe: listenTaskbarTrackInfoAlignmentChange,
    failureMessage: '初始化歌曲信息对齐方式失败',
  },
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
)
const scrolling = useEventState(
  {
    read: getTaskbarTrackInfoScrolling,
    subscribe: listenTaskbarTrackInfoScrollingChange,
    failureMessage: '初始化歌名滚动配置失败',
  },
  { ...DEFAULT_TASKBAR_TRACK_INFO_SCROLLING },
)
const visible = useEventState(
  {
    read: getTaskbarTrackInfoVisible,
    subscribe: listenTaskbarTrackInfoVisibleChange,
    failureMessage: '初始化歌曲信息显隐失败',
  },
  true,
)
</script>

<template>
  <div
    v-if="visible"
    class="flex min-w-0 flex-1 flex-col justify-center overflow-hidden leading-tight"
    :class="alignment === 'right' ? 'items-end text-right' : 'items-start text-left'"
  >
    <ScrollingTrackTitle
      class="text-sm font-medium text-(--taskbar-active-foreground)"
      :text="props.title"
      :scrolling="scrolling"
      :active="active"
    />
    <span class="max-w-full truncate text-xs text-(--taskbar-active-secondary-foreground)">{{
      props.artist
    }}</span>
  </div>
</template>
