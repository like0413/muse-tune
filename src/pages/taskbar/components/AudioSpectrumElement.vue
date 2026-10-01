<script setup lang="ts">
import { useDevicePixelRatio, useResizeObserver, useThrottleFn } from '@vueuse/core'
import type { CSSProperties } from 'vue'

import { MEDIA_SPECTRUM_SOURCE_BAND_COUNT } from '@/features/media/spectrum'
import { useAudioSpectrum } from '@/features/media/useAudioSpectrum'
import type { TaskbarAudioSpectrumSettings } from '@/features/settings/audio-spectrum'
import { useTaskbarPlaybackClock } from '@/features/taskbar/playback-clock'

const { progress } = useTaskbarPlaybackClock()

const MAX_BAR_GAP = 2
const SPECTRUM_OPACITY = 0.7
const SPECTRUM_CONTRAST_OPACITY = 0.7

const props = defineProps<{
  settings: Readonly<TaskbarAudioSpectrumSettings>
  themeColor: string
  foregroundColor: string
  overlapsProgressGradient: boolean
}>()

const settings = toRef(props, 'settings')
const { sourceBands } = useAudioSpectrum(settings)
const wrapper = useTemplateRef<HTMLDivElement>('wrapper')
const canvas = useTemplateRef<HTMLCanvasElement>('canvas')
const taskbarContainer = shallowRef<HTMLElement | null>(null)
const { pixelRatio } = useDevicePixelRatio()
const smoothedLevels = new Float32Array(MEDIA_SPECTRUM_SOURCE_BAND_COUNT)
let logicalWidth = 0
let logicalHeight = 0
let taskbarWidth = 0
let spectrumOffsetLeft = 0
let contrastColor = ''
let context: CanvasRenderingContext2D | null = null
let lastDrawnFrameWasSilent = false
let lastProcessedBands: readonly number[] | undefined
let levelsNeedUpdate = true
let pathNeedsUpdate = true
let spectrumPath: Path2D | undefined

/** 频谱只设置低频变化的布局属性，音频帧不会触发 Vue 模板更新。 */
const wrapperStyle = computed<CSSProperties>(() => {
  const color = `color-mix(in srgb, ${props.themeColor} 68%, ${props.foregroundColor} 32%)`
  return { width: `${settings.value.width}px`, color }
})

/** 按设备像素比同步画布缓冲区，避免缩放或高 DPI 下模糊。 */
function resizeCanvas(width: number, height: number) {
  const element = canvas.value
  if (!element || width <= 0 || height <= 0) return

  if (logicalWidth !== width || logicalHeight !== height) pathNeedsUpdate = true
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

/** 音频或几何变化时重建柱形路径；进度边界重绘直接复用它。 */
function createSpectrumPath(): Path2D {
  const count = settings.value.barCount
  const gap = Math.min(MAX_BAR_GAP, logicalWidth / (count * 3))
  const barWidth = Math.max(0.5, (logicalWidth - gap * (count - 1)) / count)
  const radius = Math.min(2, barWidth / 2)
  const centered = settings.value.alignment === 'center'
  const path = new Path2D()
  for (let index = 0; index < count; index += 1) {
    const level = smoothedLevels[index] ?? 0
    const height = level <= 0.002 ? 0 : Math.max(1 / pixelRatio.value, logicalHeight * level)
    if (height <= 0) continue
    const x = index * (barWidth + gap)
    const y = centered ? (logicalHeight - height) / 2 : logicalHeight - height
    path.roundRect(x, y, barWidth, height, radius)
  }
  return path
}

function getPlayedBoundary(): number {
  if (!props.overlapsProgressGradient) return 0
  const availableWidth = taskbarWidth || logicalWidth
  const progressRatio = Math.min(100, Math.max(0, progress.value)) / 100
  return Math.min(logicalWidth, Math.max(0, availableWidth * progressRatio - spectrumOffsetLeft))
}

/** 每份新音频输入只推进一次平滑；主题、进度和尺寸重绘不改变音频状态。 */
function updateLevels() {
  const currentSettings = settings.value
  const bands = sourceBands.value
  if (bands === lastProcessedBands && !levelsNeedUpdate) return
  lastProcessedBands = bands
  levelsNeedUpdate = false
  pathNeedsUpdate = true
  const count = currentSettings.barCount
  const smoothing = currentSettings.smoothing / 100
  const sensitivity = currentSettings.sensitivity / 100
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
    const targetLevel = Math.min(1, level * sensitivity)
    smoothedLevels[index] = smoothedLevels[index]! * smoothing + targetLevel * (1 - smoothing)
  }
}

/** 绘制音频柱与进度对比色，只在输入改变时重新聚合和构造路径。 */
function drawSpectrum() {
  const element = canvas.value
  if (!element || logicalWidth <= 0 || logicalHeight <= 0) return
  // 保留透明叠加；desynchronized 是低延迟提示，实际采用情况由浏览器决定。
  context ??= element.getContext('2d', { alpha: true, desynchronized: true })
  if (!context) return

  const frameIsSilent = sourceBands.value.every((level) => level === 0)
  if (frameIsSilent && lastDrawnFrameWasSilent) return
  lastDrawnFrameWasSilent = frameIsSilent
  if (frameIsSilent) {
    smoothedLevels.fill(0)
    spectrumPath = undefined
    pathNeedsUpdate = true
  } else {
    updateLevels()
    if (pathNeedsUpdate || !spectrumPath) {
      spectrumPath = createSpectrumPath()
      pathNeedsUpdate = false
    }
  }

  const ratio = pixelRatio.value
  // 缓冲区尺寸会取整；按物理像素完整清空，避免非整数 DPR 在右侧留下半列旧像素。
  context.resetTransform()
  context.clearRect(0, 0, element.width, element.height)
  context.setTransform(ratio, 0, 0, ratio, 0, 0)
  if (frameIsSilent || !spectrumPath) return
  const path = spectrumPath
  context.fillStyle = props.themeColor
  context.globalAlpha = SPECTRUM_OPACITY
  context.fill(path)

  const playedBoundary = getPlayedBoundary()
  if (playedBoundary > 0 && contrastColor) {
    // 复用已构建的柱形路径，只改变填充色和裁剪区域。
    context.save()
    context.beginPath()
    context.rect(0, 0, playedBoundary, logicalHeight)
    context.clip()
    context.fillStyle = contrastColor
    context.globalAlpha = SPECTRUM_CONTRAST_OPACITY
    context.fill(path)
    context.restore()
  }
  context.globalAlpha = 1
}

/** 合并音频帧和进度变化，按用户选择的频谱帧率节流 Canvas 重绘。 */
const requestDraw = useThrottleFn(
  () => {
    if (settings.value.visible) drawSpectrum()
  },
  () => 1000 / settings.value.frameRate,
  true,
  true,
)

/** 仅在尺寸变化时缓存频谱相对任务栏的位置，音频帧不读取布局。 */
function refreshLayoutContext() {
  // 只有竖向渐变需要频谱的相对坐标，普通模式无需读取布局。
  if (!props.overlapsProgressGradient) return
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
// 分界线在频谱之外时保持 0 或全宽，无需跟随整条任务栏的进度重复重绘。
watch(getPlayedBoundary, requestDraw, { flush: 'sync' })
// DPR 改变只需重建 Canvas 缓冲区；逻辑尺寸已由 ResizeObserver 缓存，无需再次读取 DOM。
watch(
  pixelRatio,
  () => {
    pathNeedsUpdate = true
    resizeCanvas(logicalWidth, logicalHeight)
  },
  { flush: 'post' },
)
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
  () => [settings.value.barCount, settings.value.sensitivity, settings.value.smoothing],
  () => {
    levelsNeedUpdate = true
    requestDraw()
  },
  { flush: 'sync' },
)
watch(
  () => [settings.value.alignment, props.overlapsProgressGradient],
  () => {
    pathNeedsUpdate = true
    refreshLayoutContext()
    requestDraw()
  },
  { flush: 'post' },
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
