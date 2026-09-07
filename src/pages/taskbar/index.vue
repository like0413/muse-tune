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
  DEFAULT_TASKBAR_PROGRESS_STYLE,
  getTaskbarProgressStyle,
  listenTaskbarProgressStyleChange,
  type TaskbarProgressStyle,
} from '@/features/settings/progress-style'
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
const thumbnailDataUrl = computed(() => mediaSession.value?.metadata.thumbnailDataUrl ?? null)
const { progressColor } = useTaskbarProgressColor(thumbnailDataUrl)

const backgroundTransparency = shallowRef(0)
const progressStyle = shallowRef<TaskbarProgressStyle>(DEFAULT_TASKBAR_PROGRESS_STYLE)
const elementOrder = shallowRef<TaskbarElement[]>([...DEFAULT_TASKBAR_ELEMENT_ORDER])
const progress = shallowRef(42)
let unlistenBackgroundTransparencyChange: UnlistenFn | undefined
let unlistenProgressStyleChange: UnlistenFn | undefined
let unlistenElementOrderChange: UnlistenFn | undefined

/** 仅改变页面背景 Alpha，避免文字和控件随窗口一起变淡。 */
const backgroundStyle = computed(() => ({
  backgroundColor: `color-mix(in srgb, var(--taskbar-background) ${100 - backgroundTransparency.value}%, transparent)`,
}))

/** 仅把解析后的主题色暴露给进度条，避免影响全局 primary 色。 */
const progressColorStyle = computed(() => ({
  '--taskbar-progress-color': progressColor.value,
}))

/** 计算底部横条已经播放部分的宽度。 */
const bottomProgressStyle = computed(() => ({
  width: `${progress.value}%`,
}))

/** 计算竖线位置及其左侧逐渐减弱的播放进度背景。 */
const verticalProgressStyle = computed(() => ({
  width: `${progress.value}%`,
  background:
    'linear-gradient(to left, color-mix(in srgb, var(--taskbar-progress-color) 32%, transparent) 0%, color-mix(in srgb, var(--taskbar-progress-color) 12%, transparent) 50%, color-mix(in srgb, var(--taskbar-progress-color) 12%, transparent) 100%)',
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
    progressStyle.value = await getTaskbarProgressStyle()
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
      v-bind="taskbarElementProps[element]"
    />

    <div
      class="pointer-events-none absolute inset-0"
      role="progressbar"
      aria-label="播放进度"
      aria-valuemin="0"
      aria-valuemax="100"
      :aria-valuenow="progress"
    >
      <div
        v-if="progressStyle === 'bottom'"
        class="absolute bottom-0 left-0 h-0.5 bg-(--taskbar-progress-color)"
        :style="bottomProgressStyle"
      />
      <div v-else class="absolute inset-y-0 left-0" :style="verticalProgressStyle">
        <div class="absolute inset-y-0 right-0 w-px bg-(--taskbar-progress-color)" />
      </div>
    </div>
  </main>
</template>
