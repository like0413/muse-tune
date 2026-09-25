<script setup lang="ts">
import { Slider } from '@/components/ui/slider'
import type { VolumeControlTarget } from '@/features/settings/volume-control'

const props = defineProps<{
  target: VolumeControlTarget
  percentage: number
  disabled: boolean
}>()
const emit = defineEmits<{
  setLevel: [level: number]
  adjustLevel: [direction: 1 | -1]
}>()
const { t } = useI18n({ useScope: 'global' })

const sliderValue = computed<number[]>({
  get: () => [props.percentage],
  set: (value) => emit('setLevel', (value[0] ?? 0) / 100),
})

/** 在横向音量模式任意位置滚动时按 2% 调整。 */
function handleWheel(event: WheelEvent) {
  if (event.deltaY === 0) return
  emit('adjustLevel', event.deltaY < 0 ? 1 : -1)
}
</script>

<template>
  <section
    class="flex min-w-0 flex-1 items-center gap-1 self-stretch select-none"
    @wheel.prevent="handleWheel"
  >
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
    <output class="w-6 shrink-0 text-center text-xs leading-none font-medium tabular-nums">
      {{ percentage }}
    </output>
  </section>
</template>

<style scoped>
.volume-slider :deep([data-slot='slider-track']) {
  height: 6px;
  background: color-mix(in srgb, var(--taskbar-foreground) 24%, transparent);
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
