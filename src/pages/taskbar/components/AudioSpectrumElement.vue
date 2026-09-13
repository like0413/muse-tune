<script setup lang="ts">
import { useDevicePixelRatio, useResizeObserver, useThrottleFn } from '@vueuse/core'
import type { CSSProperties } from 'vue'

import { useAudioSpectrum } from '@/features/media/useAudioSpectrum'

const SOURCE_BAND_COUNT = 64
const MAX_BAR_GAP = 2
const SPECTRUM_OPACITY = 0.34
const SPECTRUM_CONTRAST_OPACITY = 0.7
const SPECTRUM_FRAME_INTERVAL_MS = 33

const props = defineProps<{
  themeColor: string
  foregroundColor: string
  progress: number
  overlapsProgressGradient: boolean
}>()

const { settings, sourceBands } = useAudioSpectrum()
const wrapper = useTemplateRef<HTMLDivElement>('wrapper')
const canvas = useTemplateRef<HTMLCanvasElement>('canvas')
const taskbarContainer = shallowRef<HTMLElement | null>(null)
const { pixelRatio } = useDevicePixelRatio()
const barHeights = new Float32Array(SOURCE_BAND_COUNT)
const smoothedLevels = new Float32Array(SOURCE_BAND_COUNT)
let logicalWidth = 0
let logicalHeight = 0
let taskbarWidth = 0
let contrastColor = ''

/** 频谱只设置低频变化的布局属性，音频帧不会触发 Vue 模板更新。 */
const canvasStyle = computed<CSSProperties>(() => ({
  width: `${settings.value.widthPercentage}%`,
  left: `${settings.value.horizontalPosition}%`,
  transform: `translateX(-${settings.value.horizontalPosition}%)`,
  color: `color-mix(in srgb, ${props.themeColor} 68%, ${props.foregroundColor} 32%)`,
}))

/** 按设备像素比同步画布缓冲区，避免缩放或高 DPI 下模糊。 */
function resizeCanvas(width: number, height: number) {
  const element = canvas.value
  if (!element || width <= 0 || height <= 0) return

  logicalWidth = width
  logicalHeight = height
  const nextWidth = Math.max(1, Math.round(width * pixelRatio.value))
  const nextHeight = Math.max(1, Math.round(height * pixelRatio.value))
  if (element.width !== nextWidth || element.height !== nextHeight) {
    element.width = nextWidth
    element.height = nextHeight
  }
  requestDraw()
}

/** 把已计算的高度追加为一条批量路径。 */
function appendSpectrumPath(
  context: CanvasRenderingContext2D,
  count: number,
  barWidth: number,
  gap: number,
  radius: number,
  centered: boolean,
) {
  context.beginPath()
  for (let index = 0; index < count; index += 1) {
    const height = barHeights[index] ?? 0
    if (height <= 0) continue
    const x = index * (barWidth + gap)
    const y = centered ? (logicalHeight - height) / 2 : logicalHeight - height
    context.roundRect(x, y, barWidth, height, radius)
  }
}

/** 计算进度分界线在当前频谱画布内的位置。 */
function getPlayedBoundary(): number {
  if (!props.overlapsProgressGradient) return 0
  const availableWidth = taskbarWidth || logicalWidth
  const positionRatio = settings.value.horizontalPosition / 100
  const wrapperOffset = Math.max(0, availableWidth - logicalWidth) * positionRatio
  const progressRatio = Math.min(100, Math.max(0, props.progress)) / 100
  return Math.min(logicalWidth, Math.max(0, availableWidth * progressRatio - wrapperOffset))
}

/** 把固定 64 频带聚合进 Canvas，并在已播放区域叠加同主题对比色阶。 */
function drawSpectrum() {
  const element = canvas.value
  const context = element?.getContext('2d')
  if (!element || !context || logicalWidth <= 0 || logicalHeight <= 0) return

  const ratio = pixelRatio.value
  const currentSettings = settings.value
  const bands = sourceBands.value
  const count = currentSettings.barCount
  const gap = Math.min(MAX_BAR_GAP, logicalWidth / (count * 3))
  const barWidth = Math.max(0.5, (logicalWidth - gap * (count - 1)) / count)
  const radius = Math.min(2, barWidth / 2)

  context.setTransform(ratio, 0, 0, ratio, 0, 0)
  context.clearRect(0, 0, logicalWidth, logicalHeight)

  for (let index = 0; index < count; index += 1) {
    const start = Math.floor((index * SOURCE_BAND_COUNT) / count)
    const end = Math.max(start + 1, Math.floor(((index + 1) * SOURCE_BAND_COUNT) / count))
    let level = 0
    for (let sourceIndex = start; sourceIndex < end; sourceIndex += 1) {
      level = Math.max(level, (bands[sourceIndex] ?? 0) / 255)
    }
    const targetLevel = Math.min(1, level * (currentSettings.sensitivity / 100))
    const smoothing = currentSettings.smoothing / 100
    const smoothedLevel = smoothedLevels[index]! * smoothing + targetLevel * (1 - smoothing)
    smoothedLevels[index] = smoothedLevel
    barHeights[index] =
      smoothedLevel <= 0.002 ? 0 : Math.max(1 / ratio, logicalHeight * smoothedLevel)
  }

  const centered = currentSettings.alignment === 'center'
  appendSpectrumPath(context, count, barWidth, gap, radius, centered)
  context.fillStyle = props.themeColor
  context.globalAlpha = SPECTRUM_OPACITY
  context.fill()

  const playedBoundary = getPlayedBoundary()
  if (playedBoundary > 0 && contrastColor) {
    context.save()
    context.beginPath()
    context.rect(0, 0, playedBoundary, logicalHeight)
    context.clip()
    appendSpectrumPath(context, count, barWidth, gap, radius, centered)
    context.fillStyle = contrastColor
    context.globalAlpha = SPECTRUM_CONTRAST_OPACITY
    context.fill()
    context.restore()
  }
  context.globalAlpha = 1
}

/** 合并音频帧和进度变化，并把 Canvas 重绘频率限制在约 30 FPS。 */
const requestDraw = useThrottleFn(drawSpectrum, SPECTRUM_FRAME_INTERVAL_MS, true, true)

/** 缓存布局宽度和浏览器解析后的对比色，避免每个音频帧读取计算样式。 */
function refreshVisualContext() {
  const element = wrapper.value
  if (!element) return
  taskbarWidth = element.parentElement?.clientWidth ?? logicalWidth
  contrastColor = getComputedStyle(element).color || props.themeColor
}

useResizeObserver(canvas, ([entry]) => {
  if (entry) resizeCanvas(entry.contentRect.width, entry.contentRect.height)
})

useResizeObserver(taskbarContainer, () => {
  refreshVisualContext()
  requestDraw()
})

watch(sourceBands, requestDraw, { flush: 'sync' })
watch(() => props.progress, requestDraw, { flush: 'sync' })
watch(() => props.overlapsProgressGradient, requestDraw, { flush: 'sync' })
watch(
  [pixelRatio, () => props.themeColor, () => props.foregroundColor],
  () => {
    nextTick(() => {
      refreshVisualContext()
      const bounds = canvas.value?.getBoundingClientRect()
      if (bounds) resizeCanvas(bounds.width, bounds.height)
    })
  },
  { flush: 'post' },
)
watch(
  () => [
    settings.value.visible,
    settings.value.widthPercentage,
    settings.value.barCount,
    settings.value.alignment,
    settings.value.horizontalPosition,
    settings.value.sensitivity,
    settings.value.smoothing,
  ],
  () => nextTick(requestDraw),
  { flush: 'post' },
)

onMounted(() => {
  taskbarContainer.value = wrapper.value?.parentElement ?? null
  refreshVisualContext()
})
</script>

<template>
  <div
    v-show="settings.visible"
    ref="wrapper"
    class="pointer-events-none absolute inset-y-1 z-1"
    :style="canvasStyle"
    aria-hidden="true"
  >
    <canvas ref="canvas" class="size-full" />
  </div>
</template>
