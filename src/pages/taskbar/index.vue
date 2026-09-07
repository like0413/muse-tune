<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { Component } from 'vue'
import { computed, onMounted, onUnmounted, shallowRef } from 'vue'

import { useMediaSession } from '@/features/media/useMediaSession'
import { useMediaSessionSelectionPolicy } from '@/features/media/useMediaSessionSelectionPolicy'
import {
  getTaskbarBackgroundTransparency,
  listenTaskbarBackgroundTransparencyChange,
} from '@/features/settings/background-transparency'
import {
  DEFAULT_TASKBAR_ELEMENT_ORDER,
  getTaskbarElementOrder,
  listenTaskbarElementOrderChange,
  type TaskbarElement,
} from '@/features/settings/element-order'
import {
  DEFAULT_TASKBAR_PROGRESS_POSITION,
  DEFAULT_TASKBAR_PROGRESS_STYLE,
  getTaskbarProgressPosition,
  getTaskbarProgressStyle,
  listenTaskbarProgressPositionChange,
  listenTaskbarProgressStyleChange,
  type TaskbarProgressPosition,
  type TaskbarProgressStyle,
} from '@/features/settings/progress-style'
import { useTaskbarAutoHide } from '@/features/taskbar/useTaskbarAutoHide'
import { useTaskbarForegroundColor } from '@/features/theme/useTaskbarForegroundColor'
import { useTaskbarProgressColor } from '@/features/theme/useTaskbarProgressColor'

import CoverElement from './components/CoverElement.vue'
import PlaybackControlsElement from './components/PlaybackControlsElement.vue'
import TrackInfoElement from './components/TrackInfoElement.vue'

const taskbarElementComponents: Record<TaskbarElement, Component> = {
  cover: CoverElement,
  'track-info': TrackInfoElement,
  controls: PlaybackControlsElement,
}

const { session: mediaSession, controlPending, control } = useMediaSession()
useMediaSessionSelectionPolicy()
useTaskbarAutoHide(mediaSession)
const thumbnailDataUrl = computed(() => mediaSession.value?.metadata.thumbnailDataUrl ?? null)
const { progressColor } = useTaskbarProgressColor(thumbnailDataUrl)

const backgroundTransparency = shallowRef(0)
const { foregroundColor } = useTaskbarForegroundColor(backgroundTransparency)
const progressStyle = shallowRef<TaskbarProgressStyle>(DEFAULT_TASKBAR_PROGRESS_STYLE)
const progressPosition = shallowRef<TaskbarProgressPosition>(DEFAULT_TASKBAR_PROGRESS_POSITION)
const elementOrder = shallowRef<TaskbarElement[]>([...DEFAULT_TASKBAR_ELEMENT_ORDER])
const progress = shallowRef(42)
let unlistenBackgroundTransparencyChange: UnlistenFn | undefined
let unlistenProgressStyleChange: UnlistenFn | undefined
let unlistenProgressPositionChange: UnlistenFn | undefined
let unlistenElementOrderChange: UnlistenFn | undefined

/** 仅改变页面背景 Alpha，高透明时由前景色策略保证内容对比度。 */
const backgroundStyle = computed(() => ({
  backgroundColor: `color-mix(in srgb, var(--taskbar-background) ${100 - backgroundTransparency.value}%, transparent)`,
  color: foregroundColor.value,
}))

/** 仅把解析后的主题色暴露给进度条，避免影响全局 primary 色。 */
const progressColorStyle = computed(() => ({
  '--taskbar-progress-color': progressColor.value,
}))

/** 计算条形进度已经播放部分的宽度。 */
const barProgressStyle = computed(() => ({
  width: `${progress.value}%`,
}))

const progressBarPositionClass = computed(() =>
  progressPosition.value === 'top' ? 'top-0' : 'bottom-0',
)

/** 计算竖线位置，并让已播放区域从起点透明渐变到当前位置的实色主题色。 */
const verticalProgressStyle = computed(() => ({
  width: `${progress.value}%`,
  background: 'linear-gradient(to right, transparent 0%, var(--taskbar-progress-color) 100%)',
}))

/** 仅向控制区传递请求状态，其余区块共享同一份只读媒体快照。 */
const taskbarElementProps = computed<Record<TaskbarElement, Record<string, unknown>>>(() => ({
  cover: { session: mediaSession.value },
  'track-info': { session: mediaSession.value },
  controls: {
    session: mediaSession.value,
    pending: controlPending.value,
    onControl: control,
  },
}))

/** 恢复背景透明度并订阅设置窗口的实时预览。 */
async function initializeBackgroundTransparency() {
  try {
    unlistenBackgroundTransparencyChange = await listenTaskbarBackgroundTransparencyChange(
      (transparency) => {
        backgroundTransparency.value = transparency
      },
    )
    backgroundTransparency.value = await getTaskbarBackgroundTransparency()
  } catch (error) {
    console.error('初始化任务栏背景透明度失败', error)
  }
}

/** 恢复播放进度样式并订阅设置窗口的实时切换。 */
async function initializeProgressStyle() {
  try {
    unlistenProgressStyleChange = await listenTaskbarProgressStyleChange((style) => {
      progressStyle.value = style
    })
    unlistenProgressPositionChange = await listenTaskbarProgressPositionChange((position) => {
      progressPosition.value = position
    })
    ;[progressStyle.value, progressPosition.value] = await Promise.all([
      getTaskbarProgressStyle(),
      getTaskbarProgressPosition(),
    ])
  } catch (error) {
    console.error('初始化播放进度样式失败', error)
  }
}

/** 恢复区块排列并订阅设置窗口的实时更新。 */
async function initializeElementOrder() {
  try {
    unlistenElementOrderChange = await listenTaskbarElementOrderChange((order) => {
      elementOrder.value = order
    })
    elementOrder.value = await getTaskbarElementOrder()
  } catch (error) {
    console.error('初始化任务栏区块顺序失败', error)
  }
}

/** 打开或唤醒设置窗口。 */
async function openSettings() {
  try {
    await invoke('open_settings_window')
  } catch (error) {
    console.error('打开设置窗口失败', error)
  }
}

onMounted(initializeBackgroundTransparency)
onMounted(initializeProgressStyle)
onMounted(initializeElementOrder)
onUnmounted(() => {
  unlistenBackgroundTransparencyChange?.()
  unlistenProgressStyleChange?.()
  unlistenProgressPositionChange?.()
  unlistenElementOrderChange?.()
})
</script>

<template>
  <main
    class="text-taskbar-foreground relative flex size-full items-center gap-2 overflow-hidden px-2 py-1 shadow-sm select-none"
    :style="[backgroundStyle, progressColorStyle]"
    aria-label="Muse Tune 任务栏播放器"
    @contextmenu.prevent="openSettings"
  >
    <component
      :is="taskbarElementComponents[element]"
      v-for="element in elementOrder"
      :key="element"
      class="relative z-10"
      v-bind="taskbarElementProps[element]"
    />

    <div
      class="pointer-events-none absolute inset-0 z-0"
      role="progressbar"
      aria-label="播放进度"
      aria-valuemin="0"
      aria-valuemax="100"
      :aria-valuenow="progress"
    >
      <div
        v-if="progressStyle === 'bottom'"
        class="absolute left-0 h-0.5 bg-(--taskbar-progress-color)"
        :class="progressBarPositionClass"
        :style="barProgressStyle"
      />
      <div v-else class="absolute inset-y-0 left-0" :style="verticalProgressStyle">
        <div class="absolute inset-y-0 right-0 w-px bg-(--taskbar-progress-color)" />
      </div>
    </div>
  </main>
</template>
