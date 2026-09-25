<script setup lang="ts">
import { Volume1, Volume2, VolumeX } from '@lucide/vue'
import { useTimeoutFn } from '@vueuse/core'
import type { CSSProperties } from 'vue'

import { Button } from '@/components/ui/button'
import { useVolumeControl } from '@/features/media/useVolumeControl'

import VolumeSliderOverlay from './VolumeSliderOverlay.vue'

const { t } = useI18n({ useScope: 'global' })

const props = defineProps<{
  themeColor: string
  foregroundColor: string
  compact: boolean
}>()
const emit = defineEmits<{ modeChange: [active: boolean] }>()
const expanded = shallowRef(false)
const triggerHovered = shallowRef(false)
const modeHovered = shallowRef(false)
const focusWithin = shallowRef(false)
const { target, volume, setLevel, adjustLevel, toggleMuted } = useVolumeControl()

const MODE_FADE_DURATION_MS = 120
const HOVER_BRIDGE_MS = 100

const percentage = computed(() => Math.round((volume.value?.level ?? 0) * 100))
const controlSize = computed(() => (props.compact ? 'icon-xs' : 'icon-sm'))
const volumeIcon = computed(() => {
  if (!volume.value) return Volume2
  if (volume.value.muted || percentage.value === 0) return VolumeX
  return percentage.value < 50 ? Volume1 : Volume2
})
const modeStyle = computed<CSSProperties>(() => ({
  color: props.foregroundColor,
  '--volume-theme-color': props.themeColor,
  '--volume-transition-duration': `${MODE_FADE_DURATION_MS}ms`,
}))

/** 更新临时音量模式，并把状态显式通知任务栏内容层。 */
function setExpanded(value: boolean) {
  if (expanded.value === value) return
  expanded.value = value
  emit('modeChange', value)
}

const { start: scheduleClose, stop: cancelClose } = useTimeoutFn(
  () => {
    if (triggerHovered.value || modeHovered.value || focusWithin.value) return
    setExpanded(false)
  },
  HOVER_BRIDGE_MS,
  { immediate: false },
)

/** 先等待歌词层切回普通模式，再用整条 Bar 展示音量控制。 */
async function openVolumeMode() {
  if (!volume.value) return
  cancelClose()
  await nextTick()
  if (!triggerHovered.value && !modeHovered.value && !focusWithin.value) return
  setExpanded(true)
}

function handleTriggerPointerEnter() {
  triggerHovered.value = true
  void openVolumeMode()
}

function handleTriggerPointerLeave() {
  triggerHovered.value = false
  scheduleClose()
}

function handleModePointerEnter() {
  modeHovered.value = true
  void openVolumeMode()
}

function handleModePointerLeave() {
  modeHovered.value = false
  scheduleClose()
}

function handleFocusIn() {
  focusWithin.value = true
  void openVolumeMode()
}

function handleFocusOut() {
  focusWithin.value = false
  scheduleClose()
}

/** 将滚轮方向转换为 2% 的当前控制对象音量步进。 */
function handleWheel(event: WheelEvent) {
  if (event.deltaY === 0) return
  adjustLevel(event.deltaY < 0 ? 1 : -1)
}

watch(volume, (value) => {
  if (!value) setExpanded(false)
})
onUnmounted(() => setExpanded(false))
</script>

<template>
  <span
    class="flex shrink-0"
    @pointerenter="handleTriggerPointerEnter"
    @pointerleave="handleTriggerPointerLeave"
    @focusin="handleFocusIn"
    @focusout="handleFocusOut"
    @wheel.prevent="handleWheel"
  >
    <Button
      variant="ghost"
      :size="controlSize"
      class="taskbar-volume-control"
      type="button"
      :aria-label="
        t(target === 'application' ? 'media.adjustPlayerVolume' : 'media.adjustSystemVolume')
      "
      :disabled="!volume"
      @click="toggleMuted"
    >
      <component :is="volumeIcon" data-icon="inline-start" />
    </Button>

    <Teleport to="body">
      <div
        class="volume-mode fixed inset-0 z-100 flex items-center gap-1 px-2 py-1"
        :class="expanded ? 'volume-mode-entered' : 'volume-mode-left'"
        :style="modeStyle"
        :aria-hidden="!expanded"
        :inert="!expanded || undefined"
        @pointerenter="handleModePointerEnter"
        @pointerleave="handleModePointerLeave"
        @focusin="handleFocusIn"
        @focusout="handleFocusOut"
      >
        <Button
          variant="ghost"
          size="icon-xs"
          class="taskbar-volume-control shrink-0"
          type="button"
          :aria-label="
            t(target === 'application' ? 'media.adjustPlayerVolume' : 'media.adjustSystemVolume')
          "
          :disabled="!volume"
          @click="toggleMuted"
        >
          <component :is="volumeIcon" class="size-4" data-icon="inline-start" />
        </Button>
        <VolumeSliderOverlay
          :target="target"
          :percentage="percentage"
          :disabled="!volume"
          @set-level="setLevel"
          @adjust-level="adjustLevel"
        />
      </div>
    </Teleport>
  </span>
</template>

<style scoped>
.taskbar-volume-control:hover {
  color: inherit;
  background-color: color-mix(in srgb, currentColor 20%, transparent);
}

.volume-mode {
  transition: opacity var(--volume-transition-duration) ease;
}

.volume-mode-left {
  pointer-events: none;
  opacity: 0;
}

.volume-mode-entered {
  opacity: 1;
}
</style>
