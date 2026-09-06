<script setup lang="ts">
import { Pause, Play, SkipBack, SkipForward } from '@lucide/vue'
import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { computed, onMounted, onUnmounted, shallowRef } from 'vue'

import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import {
  applyTaskbarOverlapPriority,
  applyTaskbarPlacement,
  getTaskbarBackgroundTransparency,
  getTaskbarOverlapPriority,
  getTaskbarPlacement,
  listenTaskbarBackgroundTransparencyChange,
} from '@/lib/settings'

const isPlaying = shallowRef(true)
const backgroundTransparency = shallowRef(0)
const progress = 42
let unlistenBackgroundTransparencyChange: UnlistenFn | undefined

/** 仅改变页面背景 Alpha，避免文字和控件随窗口一起变淡。 */
const backgroundStyle = computed(() => ({
  backgroundColor: `color-mix(in srgb, var(--taskbar-background) ${100 - backgroundTransparency.value}%, transparent)`,
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

/** 恢复播放器位置并同步到原生定位线程。 */
async function initializePlacement() {
  try {
    await Promise.all([
      applyTaskbarPlacement(await getTaskbarPlacement()),
      applyTaskbarOverlapPriority(await getTaskbarOverlapPriority()),
    ])
  } catch (error) {
    console.error('初始化任务栏播放器布局失败', error)
  }
}

/** 切换当前播放状态。 */
function togglePlayback() {
  isPlaying.value = !isPlaying.value
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
onMounted(initializePlacement)
onUnmounted(() => {
  unlistenBackgroundTransparencyChange?.()
})
</script>

<template>
  <main
    class="text-taskbar-foreground flex size-full items-center gap-2 overflow-hidden px-2 py-1 shadow-sm select-none"
    :style="backgroundStyle"
    aria-label="Muse Tune 任务栏播放器"
    @contextmenu.prevent="openSettings"
  >
    <div
      class="bg-primary text-primary-foreground grid size-8 shrink-0 place-items-center overflow-hidden rounded-md text-base font-medium"
      aria-hidden="true"
    >
      ♪
    </div>

    <div class="flex min-w-0 flex-1 flex-col gap-1">
      <div class="flex min-w-0 items-baseline gap-1.5 whitespace-nowrap">
        <span class="truncate text-xs font-semibold">Muse Tune</span>
        <span class="text-muted-foreground min-w-0 flex-1 truncate text-[10px]">Muse Tune</span>
      </div>
      <Progress :model-value="progress" class="h-0.5" aria-label="播放进度 42%" />
    </div>

    <div class="flex shrink-0" aria-label="播放控制">
      <Button variant="ghost" size="icon-sm" type="button" title="上一曲" aria-label="上一曲">
        <SkipBack data-icon="inline-start" />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        type="button"
        title="播放或暂停"
        aria-label="播放或暂停"
        @click="togglePlayback"
      >
        <Pause v-if="isPlaying" data-icon="inline-start" />
        <Play v-else data-icon="inline-start" />
      </Button>
      <Button variant="ghost" size="icon-sm" type="button" title="下一曲" aria-label="下一曲">
        <SkipForward data-icon="inline-start" />
      </Button>
    </div>
  </main>
</template>
