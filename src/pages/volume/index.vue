<script setup lang="ts">
import { Slider } from '@/components/ui/slider'
import { useApplicationVolume } from '@/features/media/useApplicationVolume'
import { useVolumePopupLifecycle } from '@/features/media/useVolumePopupLifecycle'
import { VOLUME_POPUP_TRANSITION_MS } from '@/features/media/volume-popup'

const { t } = useI18n({ useScope: 'global' })

const { volume, setLevel, adjustLevel } = useApplicationVolume()
const { themeColor, entered, publishHover } = useVolumePopupLifecycle()

const percentage = computed(() => Math.round((volume.value?.level ?? 0) * 100))
const sliderValue = computed<number[]>({
  get: () => [percentage.value],
  set: (value) => setLevel((value[0] ?? 0) / 100),
})
const popupStyle = computed(() => ({
  '--volume-theme-color': themeColor.value,
  '--volume-transition-duration': `${VOLUME_POPUP_TRANSITION_MS}ms`,
}))

/** 在音量柱任意位置滚动时按 2% 调整。 */
function handleWheel(event: WheelEvent) {
  if (event.deltaY === 0) return
  adjustLevel(event.deltaY < 0 ? 1 : -1)
}
</script>

<template>
  <main
    class="flex size-full items-end justify-center overflow-hidden select-none"
    :style="popupStyle"
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
        :aria-label="t('media.playerVolume')"
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
