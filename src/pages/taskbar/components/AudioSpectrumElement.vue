<script setup lang="ts">
import { useDevicePixelRatio, useResizeObserver, useThrottleFn } from '@vueuse/core'
import type { CSSProperties } from 'vue'

import {
  MEDIA_SPECTRUM_FRAME_INTERVAL_MS,
  MEDIA_SPECTRUM_SOURCE_BAND_COUNT,
} from '@/features/media/spectrum'
import { useAudioSpectrum } from '@/features/media/useAudioSpectrum'
import type { TaskbarAudioSpectrumSettings } from '@/features/settings/audio-spectrum'

const MAX_BAR_GAP = 2
const SPECTRUM_OPACITY = 0.7
const SPECTRUM_CONTRAST_OPACITY = 0.7

const props = defineProps<{
  settings: Readonly<TaskbarAudioSpectrumSettings>
  themeColor: string
  foregroundColor: string
  progress: number
  overlapsProgressGradient: boolean
}>()

const settings = toRef(props, 'settings')
const { sourceBands } = useAudioSpectrum(settings)
const wrapper = useTemplateRef<HTMLDivElement>('wrapper')
const canvas = useTemplateRef<HTMLCanvasElement>('canvas')
const taskbarContainer = shallowRef<HTMLElement | null>(null)
const { pixelRatio } = useDevicePixelRatio()
const barHeights = new Float32Array(MEDIA_SPECTRUM_SOURCE_BAND_COUNT)
const smoothedLevels = new Float32Array(MEDIA_SPECTRUM_SOURCE_BAND_COUNT)
let logicalWidth = 0
let logicalHeight = 0
let taskbarWidth = 0
let spectrumOffsetLeft = 0
let contrastColor = ''
let context: CanvasRenderingContext2D | null = null
let lastDrawnFrameWasSilent = false

/** 频谱只设置低频变化的布局属性，音频帧不会触发 Vue 模板更新。 */
const wrapperStyle = computed<CSSProperties>(() => {
  const color = `color-mix(in srgb, ${props.themeColor} 68%, ${props.foregroundColor} 32%)`
  return { width: `${settings.value.width}px`, color }
})

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

function getPlayedBoundary(): number {
  if (!props.overlapsProgressGradient) return 0
  const availableWidth = taskbarWidth || logicalWidth
  const progressRatio = Math.min(100, Math.max(0, props.progress)) / 100
  return Math.min(logicalWidth, Math.max(0, availableWidth * progressRatio - spectrumOffsetLeft))
}

/** 把固定 64 频带聚合进 Canvas，并在已播放区域叠加同主题对比色阶。 */
function drawSpectrum() {
  const element = canvas.value
  if (!element || logicalWidth <= 0 || logicalHeight <= 0) return
  // alpha 保留透明画布以叠在背景之上；desynchronized 让 WebView2 不必等合成器帧同步即可上屏，
  // 代价是画布可能与 DOM 短暂不一致，因此这一层只自绘、不参与与其它层的逐帧对齐。
  context ??= element.getContext('2d', { alpha: true, desynchronized: true })
  if (!context) return

  const ratio = pixelRatio.value
  const currentSettings = settings.value
  const bands = sourceBands.value
  const frameIsSilent = bands.every((level) => level === 0)
  if (frameIsSilent && lastDrawnFrameWasSilent) return
  lastDrawnFrameWasSilent = frameIsSilent
  const count = currentSettings.barCount
  // 间隙不超过每柱平均槽宽的 1/3，且最多 2px；柱宽保底 0.5、圆角不超过 2 且不超过半个柱宽，
  // 否则柱子变多或变窄时会被间隙吃光、或圆角把柱体削成非矩形。
  const gap = Math.min(MAX_BAR_GAP, logicalWidth / (count * 3))
  const barWidth = Math.max(0.5, (logicalWidth - gap * (count - 1)) / count)
  const radius = Math.min(2, barWidth / 2)

  context.setTransform(ratio, 0, 0, ratio, 0, 0)
  context.clearRect(0, 0, logicalWidth, logicalHeight)

  for (let index = 0; index < count; index += 1) {
    const start = Math.floor((index * MEDIA_SPECTRUM_SOURCE_BAND_COUNT) / count)
    const end = Math.max(
      start + 1,
      Math.floor(((index + 1) * MEDIA_SPECTRUM_SOURCE_BAND_COUNT) / count),
    )
    let level = 0
    for (let sourceIndex = start; sourceIndex < end; sourceIndex += 1) {
      level = Math.max(level, (bands[sourceIndex] ?? 0) / 255)
    }
    const targetLevel = Math.min(1, level * (currentSettings.sensitivity / 100))
    const smoothing = currentSettings.smoothing / 100
    const smoothedLevel = smoothedLevels[index]! * smoothing + targetLevel * (1 - smoothing)
    smoothedLevels[index] = smoothedLevel
    // 0.002 以下的电平直接归零：平滑是递归加权，残留的小数会永远衰减不到 0。
    // 1 / ratio 是 1 个物理像素对应的 CSS 高度，保证有能量时柱体至少可见。
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

/** 合并音频帧和进度变化，并把 Canvas 重绘频率限制在原生帧率上限。 */
const requestDraw = useThrottleFn(
  () => {
    if (settings.value.visible) drawSpectrum()
  },
  MEDIA_SPECTRUM_FRAME_INTERVAL_MS,
  true,
  true,
)

/** 仅在尺寸变化时缓存频谱相对任务栏的位置，音频帧不读取布局。 */
function refreshLayoutContext() {
  const element = wrapper.value
  if (!element) return
  const parent = element.parentElement
  taskbarWidth = parent?.clientWidth ?? logicalWidth
  spectrumOffsetLeft = parent
    ? Math.max(0, element.getBoundingClientRect().left - parent.getBoundingClientRect().left)
    : 0
}

/** 仅在主题变化时读取浏览器解析后的混合色，避免尺寸变化触发样式计算。 */
function refreshContrastColor() {
  const element = wrapper.value
  if (!element) return
  contrastColor = getComputedStyle(element).color || props.themeColor
}

useResizeObserver(canvas, ([entry]) => {
  if (entry) {
    refreshLayoutContext()
    resizeCanvas(entry.contentRect.width, entry.contentRect.height)
  }
})

useResizeObserver(taskbarContainer, () => {
  refreshLayoutContext()
  requestDraw()
})

watch(sourceBands, requestDraw, { flush: 'sync' })
// 进度只影响叠加在频谱上的已播放分界线；不叠加时重绘结果与上一帧逐像素相同。
watch(() => (props.overlapsProgressGradient ? props.progress : 0), requestDraw, { flush: 'sync' })
// DPR 改变只需重建 Canvas 缓冲区；逻辑尺寸已由 ResizeObserver 缓存，无需再次读取 DOM。
watch(pixelRatio, () => resizeCanvas(logicalWidth, logicalHeight), { flush: 'post' })
watch(
  [() => props.themeColor, () => props.foregroundColor],
  () => {
    refreshContrastColor()
    requestDraw()
  },
  { flush: 'post' },
)
// 以下参数只改变 Canvas 像素，不参与 DOM 布局。
watch(
  () => [
    settings.value.barCount,
    settings.value.alignment,
    settings.value.sensitivity,
    settings.value.smoothing,
  ],
  requestDraw,
  { flush: 'sync' },
)

onMounted(() => {
  taskbarContainer.value = wrapper.value?.parentElement ?? null
  refreshLayoutContext()
  refreshContrastColor()
})
</script>

<template>
  <div
    ref="wrapper"
    class="pointer-events-none absolute inset-y-1 right-2 z-1"
    :style="wrapperStyle"
    aria-hidden="true"
  >
    <canvas ref="canvas" class="size-full" />
  </div>
</template>
