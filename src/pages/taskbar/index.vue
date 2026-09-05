<script setup lang="ts">
import { Pause, Play, SkipBack, SkipForward } from '@lucide/vue'
import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { Effect, getCurrentWindow } from '@tauri-apps/api/window'
import { computed, onMounted, onUnmounted, shallowRef } from 'vue'

import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import {
  applyTaskbarPlacement,
  getTaskbarMaterial,
  getTaskbarPlacement,
  listenTaskbarMaterialChange,
  type TaskbarMaterial,
} from '@/lib/settings'
import { cn } from '@/lib/utils'

const isPlaying = shallowRef(true)
const material = shallowRef<TaskbarMaterial>('normal')
const progress = 42
let unlistenMaterialChange: UnlistenFn | undefined
let effectQueue = Promise.resolve()

const MATERIAL_CLASSES: Record<TaskbarMaterial, string> = {
  normal: 'bg-taskbar-background text-taskbar-foreground',
  transparent: 'bg-transparent',
  acrylic: 'bg-background/55',
}

const materialClass = computed(() => MATERIAL_CLASSES[material.value])

/** 按顺序应用系统窗口效果，避免快速切换时旧请求覆盖新选择。 */
function applyMaterial(nextMaterial: TaskbarMaterial) {
  material.value = nextMaterial
  effectQueue = effectQueue
    .then(async () => {
      const window = getCurrentWindow()

      if (nextMaterial === 'acrylic') {
        await window.setEffects({ effects: [Effect.Acrylic] })
        return
      }

      await window.clearEffects()
    })
    .catch((error) => {
      console.error('应用任务栏窗口材质失败', error)
    })
}

/** 恢复设置并订阅设置窗口的实时变更。 */
async function initializeMaterial() {
  try {
    unlistenMaterialChange = await listenTaskbarMaterialChange(applyMaterial)
    applyMaterial(await getTaskbarMaterial())
  } catch (error) {
    console.error('初始化任务栏窗口材质失败', error)
  }
}

/** 恢复播放器位置并同步到原生定位线程。 */
async function initializePlacement() {
  try {
    await applyTaskbarPlacement(await getTaskbarPlacement())
  } catch (error) {
    console.error('初始化播放器位置失败', error)
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

onMounted(initializeMaterial)
onMounted(initializePlacement)
onUnmounted(() => unlistenMaterialChange?.())
</script>

<template>
  <main
    :class="
      cn(
        'text-foreground flex size-full items-center gap-2 overflow-hidden px-2 py-1 shadow-sm select-none',
        materialClass,
      )
    "
    :data-material="material"
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
