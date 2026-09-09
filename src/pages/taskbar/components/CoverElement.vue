<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { useTimeoutFn } from '@vueuse/core'
import { computed, onMounted, onUnmounted, shallowRef, watch } from 'vue'

import { getThumbnailUpdateDebounceMs } from '@/features/media/players'
import type { MediaSessionSnapshot } from '@/features/media/types'
import {
  DEFAULT_TASKBAR_COVER_APPEARANCE,
  getTaskbarCoverAppearance,
  listenTaskbarCoverAppearanceChange,
  type TaskbarCoverAppearance,
} from '@/features/settings/cover'

import PlayerSourceBadge from './PlayerSourceBadge.vue'

const props = defineProps<{ session: MediaSessionSnapshot | null }>()
const appearance = shallowRef<TaskbarCoverAppearance>({ ...DEFAULT_TASKBAR_COVER_APPEARANCE })
const displayedThumbnail = shallowRef<string | null>(null)
const thumbnailUpdateDelayMs = shallowRef(0)
let unlistenAppearanceChange: UnlistenFn | undefined
let thumbnailRequestId = 0
let pendingThumbnail: string | null = null
const { start: scheduleThumbnailClear, stop: cancelThumbnailClear } = useTimeoutFn(
  () => {
    displayedThumbnail.value = null
  },
  500,
  { immediate: false },
)
const { start: scheduleThumbnailUpdate, stop: cancelThumbnailUpdate } = useTimeoutFn(
  () => {
    void preloadThumbnail(pendingThumbnail)
  },
  thumbnailUpdateDelayMs,
  { immediate: false },
)

/** 依据保存的形状与播放状态生成封面图层类名。 */
const shapeClass = computed(() => ({
  'rounded-none': appearance.value.shape === 'square',
  'rounded-md': appearance.value.shape === 'rounded',
  'rounded-full': appearance.value.shape === 'circle',
  'cover-rotating': appearance.value.shape === 'circle' && appearance.value.rotateWhenPlaying,
  'cover-rotation-paused':
    appearance.value.shape === 'circle' &&
    appearance.value.rotateWhenPlaying &&
    props.session?.playback.status !== 'playing',
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

/** 先解码新封面再替换当前图片，避免切歌期间短暂显示占位符。 */
async function preloadThumbnail(thumbnailDataUrl: string | null) {
  const requestId = ++thumbnailRequestId
  if (!thumbnailDataUrl) {
    if (props.session) scheduleThumbnailClear()
    else displayedThumbnail.value = null
    return
  }

  cancelThumbnailClear()
  const image = new window.Image()
  image.src = thumbnailDataUrl
  try {
    await image.decode()
  } catch {
    if (requestId === thumbnailRequestId) scheduleThumbnailClear()
    return
  }
  if (requestId === thumbnailRequestId) displayedThumbnail.value = thumbnailDataUrl
}

/** 按播放器规则合并切歌期间连续发布的封面，只解码最后一个候选。 */
function updateThumbnail(
  player: MediaSessionSnapshot['player'] | null,
  thumbnailDataUrl: string | null,
) {
  cancelThumbnailUpdate()
  const delayMs = player ? getThumbnailUpdateDebounceMs(player) : 0
  if (delayMs <= 0 || !thumbnailDataUrl) {
    void preloadThumbnail(thumbnailDataUrl)
    return
  }
  pendingThumbnail = thumbnailDataUrl
  thumbnailUpdateDelayMs.value = delayMs
  scheduleThumbnailUpdate()
}

watch(
  () => [props.session?.player ?? null, props.session?.metadata.thumbnailDataUrl ?? null] as const,
  ([player, thumbnailDataUrl]) => updateThumbnail(player, thumbnailDataUrl),
  { immediate: true },
)

onMounted(initializeAppearance)
onUnmounted(() => {
  thumbnailRequestId += 1
  cancelThumbnailClear()
  cancelThumbnailUpdate()
  unlistenAppearanceChange?.()
})
</script>

<template>
  <div v-if="appearance.visible" class="relative size-8 shrink-0" aria-hidden="true">
    <div
      class="bg-primary text-primary-foreground grid size-full place-items-center overflow-hidden text-base font-medium"
      :class="shapeClass"
    >
      <img
        v-if="displayedThumbnail"
        class="size-full object-cover"
        :src="displayedThumbnail"
        alt=""
      />
      <span v-else>♪</span>
    </div>
    <PlayerSourceBadge
      v-if="appearance.showPlayerSource && session"
      :player="session.player"
      :icon-data-url="session.sourceIconDataUrl ?? null"
    />
  </div>
</template>

<style scoped>
/* 旋转只作用于封面图层，右下角播放器来源徽标保持静止。 */
.cover-rotating {
  animation: cover-rotation 12s linear infinite;
}

.cover-rotation-paused {
  animation-play-state: paused;
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
