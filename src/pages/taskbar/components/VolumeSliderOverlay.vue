<script setup lang="ts">
import { Volume1, Volume2, VolumeX } from '@lucide/vue'

import { Button } from '@/components/ui/button'
import { Slider } from '@/components/ui/slider'
import type { VolumeControlTarget } from '@/features/settings/volume-control'

const props = defineProps<{
  target: VolumeControlTarget
  percentage: number
  muted: boolean
  themeColor: string
  disabled: boolean
}>()
const emit = defineEmits<{
  setLevel: [level: number]
  adjustLevel: [direction: 1 | -1]
  toggleMuted: []
}>()
const { t } = useI18n({ useScope: 'global' })

const sliderValue = computed<number[]>({
  get: () => [props.percentage],
  set: (value) => emit('setLevel', (value[0] ?? 0) / 100),
})
const overlayStyle = computed(() => ({ '--volume-theme-color': props.themeColor }))
const volumeIcon = computed(() => {
  if (props.muted || props.percentage === 0) return VolumeX
  return props.percentage < 50 ? Volume1 : Volume2
})

/** 弹层覆盖 bar 后由自身消费滚轮，避免事件冒泡造成双倍步进。 */
function handleWheel(event: WheelEvent) {
  if (event.deltaY === 0) return
  emit('adjustLevel', event.deltaY < 0 ? 1 : -1)
}
</script>

<template>
  <section
    class="absolute inset-0 z-100 flex items-center gap-2 px-2 py-1 select-none"
    :style="overlayStyle"
    @wheel.prevent.stop="handleWheel"
  >
    <Button
      variant="ghost"
      size="icon-xs"
      class="taskbar-volume-control shrink-0"
      type="button"
      :aria-label="
        t(target === 'application' ? 'media.adjustPlayerVolume' : 'media.adjustSystemVolume')
      "
      :disabled="disabled"
      @click="emit('toggleMuted')"
    >
      <component :is="volumeIcon" class="size-4" data-icon="inline-start" />
    </Button>
    <Slider
      v-model="sliderValue"
      orientation="horizontal"
      :min="0"
      :max="100"
      :step="1"
      :disabled="disabled"
      :aria-label="t(target === 'application' ? 'media.playerVolume' : 'media.systemVolume')"
      class="volume-slider min-w-0 flex-1"
    />
    <output class="w-7 shrink-0 text-center text-xs leading-none font-medium tabular-nums">
      {{ percentage }}
    </output>
  </section>
</template>

<style scoped>
.taskbar-volume-control:hover {
  color: inherit;
  background-color: color-mix(in srgb, currentColor 20%, transparent);
}

.volume-slider :deep([data-slot='slider-track']) {
  height: 6px;
  background: color-mix(in srgb, var(--taskbar-active-foreground) 24%, transparent);
}

.volume-slider :deep([data-slot='slider-range']) {
  background: var(--volume-theme-color);
}

.volume-slider :deep([data-slot='slider-thumb']) {
  width: 16px;
  height: 16px;
  border-color: var(--volume-theme-color);
  box-shadow: 0 1px 4px oklch(0 0 0 / 24%);
}
</style>
