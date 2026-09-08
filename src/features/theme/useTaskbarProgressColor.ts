import type { UnlistenFn } from '@tauri-apps/api/event'
import { FastAverageColor } from 'fast-average-color'
import type { ComputedRef } from 'vue'
import { computed, onMounted, onUnmounted, shallowRef, watch } from 'vue'

import {
  DEFAULT_TASKBAR_THEME_COLOR,
  getTaskbarThemeColor,
  listenTaskbarThemeColorChange,
  type TaskbarThemeColor,
} from '@/features/settings/theme-color'
import { getSystemAccentColor, listenSystemAccentColorChange } from '@/features/system/accent-color'
import { colorMode } from '@/lib/color-mode'

const FALLBACK_PROGRESS_COLOR = '#1677ff'
const COVER_COLOR_FALLBACK = {
  dark: { background: [32, 32, 32], hex: '#8ab4f8', rgba: [138, 180, 248, 255] },
  light: { background: [243, 243, 243], hex: '#1677ff', rgba: [22, 119, 255, 255] },
} as const
const MINIMUM_GRAPHIC_CONTRAST_RATIO = 3
const SIMILAR_BRIGHTNESS_THRESHOLD = 150

/** 按 WCAG 相对亮度公式计算单个 sRGB 分量。 */
function linearizeSrgb(component: number): number {
  const value = component / 255
  return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
}

/** 计算 RGB 颜色的相对亮度。 */
function relativeLuminance(color: readonly number[]): number {
  return (
    0.2126 * linearizeSrgb(color[0] ?? 0) +
    0.7152 * linearizeSrgb(color[1] ?? 0) +
    0.0722 * linearizeSrgb(color[2] ?? 0)
  )
}

/** 判断主题色与 bar 背景是否达到非文本图形建议的 3:1 对比度。 */
function hasClearContrast(color: readonly number[], background: readonly number[]): boolean {
  const colorLuminance = relativeLuminance(color)
  const backgroundLuminance = relativeLuminance(background)
  const lighter = Math.max(colorLuminance, backgroundLuminance)
  const darker = Math.min(colorLuminance, backgroundLuminance)
  return (lighter + 0.05) / (darker + 0.05) >= MINIMUM_GRAPHIC_CONTRAST_RATIO
}

/** 按设置来源解析进度条颜色，并只在封面变化时重新取色。 */
export function useTaskbarProgressColor(thumbnailDataUrl: ComputedRef<string | null>) {
  const setting = shallowRef<TaskbarThemeColor>({ ...DEFAULT_TASKBAR_THEME_COLOR })
  const systemColor = shallowRef(FALLBACK_PROGRESS_COLOR)
  const coverColor = shallowRef<string | null>(null)
  let colorExtractor: FastAverageColor | undefined
  let extractionRequestId = 0
  let settingRevision = 0
  let systemColorRevision = 0
  let disposed = false
  let unlistenSetting: UnlistenFn | undefined
  let unlistenSystemColor: UnlistenFn | undefined

  /** 异步提取封面主色；新颜色产出前保留上一个有效结果，避免切歌闪色。 */
  async function extractCoverColor() {
    const thumbnail = thumbnailDataUrl.value
    const requestId = ++extractionRequestId
    if (setting.value.source !== 'cover' || !thumbnail) return

    try {
      colorExtractor ??= new FastAverageColor()
      const darkBar = colorMode.state.value === 'dark'
      const fallback = darkBar ? COVER_COLOR_FALLBACK.dark : COVER_COLOR_FALLBACK.light
      const originalResult = await colorExtractor.getColorAsync(thumbnail, {
        algorithm: 'dominant',
        mode: 'speed',
        silent: true,
      })
      if (requestId !== extractionRequestId || originalResult.error) return
      if (hasClearContrast(originalResult.value, fallback.background)) {
        coverColor.value = originalResult.hex
        return
      }

      const contrastResult = await colorExtractor.getColorAsync(thumbnail, {
        algorithm: 'dominant',
        mode: 'speed',
        // 仅在原始主色不清楚时排除同明暗方向像素，正常封面保持原始取色结果。
        ignoredColor: darkBar
          ? [0, 0, 0, 255, SIMILAR_BRIGHTNESS_THRESHOLD]
          : [255, 255, 255, 255, SIMILAR_BRIGHTNESS_THRESHOLD],
        defaultColor: [...fallback.rgba],
        silent: true,
      })
      if (requestId === extractionRequestId && !contrastResult.error) {
        coverColor.value = hasClearContrast(contrastResult.value, fallback.background)
          ? contrastResult.hex
          : fallback.hex
      }
    } catch (error) {
      if (requestId === extractionRequestId) {
        console.error('提取封面主色失败', error)
      }
    }
  }

  /** 初始化设置、系统色以及两个实时变化事件。 */
  async function initialize() {
    try {
      const stopSettingListener = await listenTaskbarThemeColorChange((value) => {
        settingRevision += 1
        setting.value = value
        void extractCoverColor()
      })
      if (disposed) {
        stopSettingListener()
        return
      }
      unlistenSetting = stopSettingListener

      const stopSystemColorListener = await listenSystemAccentColorChange((color) => {
        systemColorRevision += 1
        systemColor.value = color
      })
      if (disposed) {
        stopSystemColorListener()
        return
      }
      unlistenSystemColor = stopSystemColorListener

      const settingRevisionBeforeRead = settingRevision
      const systemColorRevisionBeforeRead = systemColorRevision
      const [savedSetting, accentColor] = await Promise.all([
        getTaskbarThemeColor(),
        getSystemAccentColor(),
      ])
      if (disposed) return
      if (settingRevision === settingRevisionBeforeRead) setting.value = savedSetting
      if (systemColorRevision === systemColorRevisionBeforeRead) systemColor.value = accentColor
      await extractCoverColor()
    } catch (error) {
      console.error('初始化任务栏主题色失败', error)
    }
  }

  const progressColor = computed(() => {
    switch (setting.value.source) {
      case 'cover':
        return coverColor.value ?? systemColor.value
      case 'custom':
        return setting.value.customColor
      default:
        return systemColor.value
    }
  })

  watch(thumbnailDataUrl, () => void extractCoverColor())
  watch(colorMode.state, () => void extractCoverColor())
  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    extractionRequestId += 1
    unlistenSetting?.()
    unlistenSystemColor?.()
    colorExtractor?.destroy()
  })

  return { progressColor }
}
