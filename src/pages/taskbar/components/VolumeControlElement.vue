<script setup lang="ts">
import { Volume1, Volume2, VolumeX } from '@lucide/vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { emitTo, listen } from '@tauri-apps/api/event'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'

import { Button } from '@/components/ui/button'
import { reportBackgroundFailure } from '@/features/feedback/errors'
import { useApplicationVolume } from '@/features/media/useApplicationVolume'
import {
  VOLUME_POPUP_CLOSE_EVENT,
  VOLUME_POPUP_HOVER_BRIDGE_MS,
  VOLUME_POPUP_HOVER_CHANGED_EVENT,
  VOLUME_POPUP_LABEL,
  type VolumePopupHoverPayload,
} from '@/features/media/volume-popup'
import { showVolumePopup } from '@/features/taskbar/client'

const { t } = useI18n({ useScope: 'global' })

const props = defineProps<{
  themeColor: string
  compact: boolean
}>()
const anchor = useTemplateRef<HTMLElement>('anchor')
const triggerHovered = shallowRef(false)
const popupHovered = shallowRef(false)
const currentWindow = getCurrentWebviewWindow()
const { volume, adjustLevel, toggleMuted } = useApplicationVolume()
let closeTimer: ReturnType<typeof setTimeout> | undefined
let unlistenPopupHover: UnlistenFn | undefined
let disposed = false

const percentage = computed(() => Math.round((volume.value?.level ?? 0) * 100))
const controlSize = computed(() => (props.compact ? 'icon-xs' : 'icon-sm'))
const volumeIcon = computed(() => {
  if (!volume.value) return Volume2
  if (volume.value.muted || percentage.value === 0) return VolumeX
  return percentage.value < 50 ? Volume1 : Volume2
})

/** 取消跨原生窗口移动期间的延迟关闭。 */
function cancelClose() {
  if (closeTimer) clearTimeout(closeTimer)
  closeTimer = undefined
}

/** 两个 hover 区域都离开后通知悬浮窗播放离场动画。 */
function scheduleClose() {
  cancelClose()
  closeTimer = setTimeout(() => {
    if (triggerHovered.value || popupHovered.value) return
    void emitTo(VOLUME_POPUP_LABEL, VOLUME_POPUP_CLOSE_EVENT, {
      ownerLabel: currentWindow.label,
    })
  }, VOLUME_POPUP_HOVER_BRIDGE_MS)
}

async function openPopup() {
  if (!anchor.value || !volume.value) return
  cancelClose()
  const bounds = anchor.value.getBoundingClientRect()
  try {
    await showVolumePopup(bounds.left + bounds.width / 2, props.themeColor)
  } catch (error) {
    reportBackgroundFailure('显示音量悬浮窗失败', error)
  }
}

/** 记录按钮 hover，确保按钮与音量柱之间可连续移动。 */
function handlePointerEnter() {
  triggerHovered.value = true
  void openPopup()
}

/** 延迟关闭，为鼠标进入独立音量窗留出跨窗时间。 */
function handlePointerLeave() {
  triggerHovered.value = false
  scheduleClose()
}

/** 将滚轮方向转换为 2% 的应用音量步进。 */
function handleWheel(event: WheelEvent) {
  if (event.deltaY === 0) return
  adjustLevel(event.deltaY < 0 ? 1 : -1)
}

onMounted(async () => {
  const stopListener = await listen<VolumePopupHoverPayload>(
    VOLUME_POPUP_HOVER_CHANGED_EVENT,
    ({ payload }) => {
      if (payload.ownerLabel !== currentWindow.label) return
      popupHovered.value = payload.hovered
      if (payload.hovered) cancelClose()
      else scheduleClose()
    },
  )
  if (disposed) stopListener()
  else unlistenPopupHover = stopListener
})

onUnmounted(() => {
  disposed = true
  cancelClose()
  unlistenPopupHover?.()
  void emitTo(VOLUME_POPUP_LABEL, VOLUME_POPUP_CLOSE_EVENT, {
    ownerLabel: currentWindow.label,
  })
})
</script>

<template>
  <span
    ref="anchor"
    class="flex shrink-0"
    @pointerenter="handlePointerEnter"
    @pointerleave="handlePointerLeave"
    @wheel.prevent="handleWheel"
  >
    <Button
      variant="ghost"
      :size="controlSize"
      class="taskbar-volume-control"
      type="button"
      :aria-label="t('media.adjustVolume')"
      :disabled="!volume"
      @click="toggleMuted"
    >
      <component :is="volumeIcon" data-icon="inline-start" />
    </Button>
  </span>
</template>

<style scoped>
.taskbar-volume-control:hover {
  color: inherit;
  background-color: color-mix(in srgb, currentColor 20%, transparent);
}
</style>
