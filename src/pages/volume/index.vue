<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { Slider } from '@/components/ui/slider'
import { useApplicationVolume } from '@/features/media/useApplicationVolume'
import {
  VOLUME_POPUP_CLOSE_EVENT,
  VOLUME_POPUP_HOVER_CHANGED_EVENT,
  VOLUME_POPUP_OPEN_EVENT,
  VOLUME_POPUP_TRANSITION_MS,
  type VolumePopupOpenPayload,
  type VolumePopupOwnerPayload,
} from '@/features/media/volume-popup'
import { hideVolumePopup } from '@/features/taskbar/client'

const { volume, setLevel, adjustLevel } = useApplicationVolume()
const ownerLabel = shallowRef('')
const themeColor = shallowRef('#1677ff')
const generation = shallowRef(0)
const entered = shallowRef(false)
let hideTimer: ReturnType<typeof setTimeout> | undefined
let animationFrame: number | undefined
let unlistenOpen: UnlistenFn | undefined
let unlistenClose: UnlistenFn | undefined
let disposed = false

const percentage = computed(() => Math.round((volume.value?.level ?? 0) * 100))
const sliderValue = computed<number[]>({
  get: () => [percentage.value],
  set: (value) => setLevel((value[0] ?? 0) / 100),
})
const popupStyle = computed(() => ({
  '--volume-theme-color': themeColor.value,
  '--volume-transition-duration': `${VOLUME_POPUP_TRANSITION_MS}ms`,
}))

/** 清除尚未执行的动画调度。 */
function clearAnimationTimers() {
  if (hideTimer) clearTimeout(hideTimer)
  if (animationFrame !== undefined) cancelAnimationFrame(animationFrame)
  hideTimer = undefined
  animationFrame = undefined
}

/** 重置离场状态并在下一帧播放等时长进入动画。 */
function open(payload: VolumePopupOpenPayload) {
  clearAnimationTimers()
  ownerLabel.value = payload.ownerLabel
  themeColor.value = payload.themeColor
  generation.value = payload.generation
  entered.value = false
  animationFrame = requestAnimationFrame(() => {
    entered.value = true
    animationFrame = undefined
  })
}

/** 播放离场动画，结束后再隐藏原生窗，进入与离开时长一致。 */
function close(payload: VolumePopupOwnerPayload) {
  if (payload.ownerLabel !== ownerLabel.value) return
  clearAnimationTimers()
  entered.value = false
  const closingGeneration = generation.value
  hideTimer = setTimeout(async () => {
    hideTimer = undefined
    try {
      await hideVolumePopup(closingGeneration)
    } catch (error) {
      console.error('隐藏音量悬浮窗失败', error)
    }
  }, VOLUME_POPUP_TRANSITION_MS)
}

/** 将整个悬浮柱纳入同一 hover 区域。 */
function publishHover(hovered: boolean) {
  if (!ownerLabel.value) return
  void emit(VOLUME_POPUP_HOVER_CHANGED_EVENT, {
    ownerLabel: ownerLabel.value,
    hovered,
  })
}

/** 在音量柱任意位置滚动时按 2% 调整。 */
function handleWheel(event: WheelEvent) {
  if (event.deltaY === 0) return
  adjustLevel(event.deltaY < 0 ? 1 : -1)
}

onMounted(async () => {
  const listeners = await Promise.all([
    listen<VolumePopupOpenPayload>(VOLUME_POPUP_OPEN_EVENT, ({ payload }) => open(payload)),
    listen<VolumePopupOwnerPayload>(VOLUME_POPUP_CLOSE_EVENT, ({ payload }) => close(payload)),
  ])
  if (disposed) listeners.forEach((stopListener) => stopListener())
  else [unlistenOpen, unlistenClose] = listeners
})

onUnmounted(() => {
  disposed = true
  clearAnimationTimers()
  unlistenOpen?.()
  unlistenClose?.()
})
</script>

<template>
  <main
    class="flex size-full items-end justify-center overflow-hidden select-none"
    :style="popupStyle"
    aria-label="播放器音量"
    @pointerenter="publishHover(true)"
    @pointerleave="publishHover(false)"
    @wheel.prevent="handleWheel"
  >
    <section
      class="volume-card flex size-full flex-col items-center rounded-md py-3 shadow-lg"
      :class="entered ? 'volume-card-entered' : 'volume-card-left'"
    >
      <Slider
        v-model="sliderValue"
        orientation="vertical"
        :min="0"
        :max="100"
        :step="1"
        :disabled="!volume"
        aria-label="播放器音量"
        class="volume-slider min-h-0! flex-1"
      />
      <output class="mt-2 text-xs leading-none font-medium tabular-nums">{{ percentage }}</output>
    </section>
  </main>
</template>

<style scoped>
.volume-card {
  position: relative;
  background: color-mix(in srgb, var(--taskbar-background) 96%, transparent);
  color: var(--taskbar-foreground);
  transition:
    opacity var(--volume-transition-duration) ease,
    transform var(--volume-transition-duration) ease;
  transform-origin: bottom center;
}

.volume-card::after {
  position: absolute;
  bottom: -5px;
  left: 50%;
  width: 10px;
  height: 10px;
  content: '';
  background: inherit;
  border-radius: 2px;
  transform: translateX(-50%) rotate(45deg);
}

.volume-card-left {
  opacity: 0;
  transform: translateY(6px) scale(0.96);
}

.volume-card-entered {
  opacity: 1;
  transform: translateY(0) scale(1);
}

.volume-slider :deep([data-slot='slider-track']) {
  width: 6px;
  background: color-mix(in srgb, var(--taskbar-foreground) 24%, transparent);
}

.volume-slider :deep([data-slot='slider-range']) {
  background: var(--volume-theme-color);
}

.volume-slider :deep([data-slot='slider-thumb']) {
  width: 16px;
  height: 16px;
  border-color: var(--volume-theme-color);
  box-shadow: 0 1px 4px rgb(0 0 0 / 24%);
}
</style>
