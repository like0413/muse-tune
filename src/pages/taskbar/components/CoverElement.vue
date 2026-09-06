<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { computed, onMounted, onUnmounted, shallowRef } from 'vue'

import {
  DEFAULT_TASKBAR_COVER_APPEARANCE,
  getTaskbarCoverAppearance,
  listenTaskbarCoverAppearanceChange,
  type TaskbarCoverAppearance,
} from '@/features/settings/cover'

const props = withDefaults(defineProps<{ playing?: boolean }>(), { playing: false })
const appearance = shallowRef<TaskbarCoverAppearance>({ ...DEFAULT_TASKBAR_COVER_APPEARANCE })
let unlistenAppearanceChange: UnlistenFn | undefined

/** 依据保存的形状生成稳定类名，避免改变封面占位尺寸。 */
const shapeClass = computed(() => ({
  'rounded-none': appearance.value.shape === 'square',
  'rounded-md': appearance.value.shape === 'rounded',
  'rounded-full': appearance.value.shape === 'circle',
  'cover-rotating':
    appearance.value.shape === 'circle' && appearance.value.rotateWhenPlaying && props.playing,
}))

/** 恢复封面配置，并接收设置窗口的实时更新。 */
async function initializeAppearance() {
  try {
    unlistenAppearanceChange = await listenTaskbarCoverAppearanceChange((value) => {
      appearance.value = value
    })
    appearance.value = await getTaskbarCoverAppearance()
  } catch (error) {
    console.error('初始化封面配置失败', error)
  }
}

onMounted(initializeAppearance)
onUnmounted(() => unlistenAppearanceChange?.())
</script>

<template>
  <div
    v-if="appearance.visible"
    class="bg-primary text-primary-foreground grid size-8 shrink-0 place-items-center overflow-hidden text-base font-medium"
    :class="shapeClass"
    aria-hidden="true"
  >
    ♪
  </div>
</template>

<style scoped>
/* 预留媒体播放状态接口；只旋转圆形封面，并尊重系统减少动态效果偏好。 */
.cover-rotating {
  animation: cover-rotation 12s linear infinite;
}

@keyframes cover-rotation {
  to {
    transform: rotate(1turn);
  }
}

@media (prefers-reduced-motion: reduce) {
  .cover-rotating {
    animation: none;
  }
}
</style>
