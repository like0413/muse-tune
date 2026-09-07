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

const FALLBACK_PROGRESS_COLOR = '#1677ff'

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

  /** 异步提取封面主色；请求编号防止切歌时旧结果覆盖新封面。 */
  async function extractCoverColor() {
    const thumbnail = thumbnailDataUrl.value
    const requestId = ++extractionRequestId
    if (setting.value.source !== 'cover' || !thumbnail) {
      coverColor.value = null
      return
    }

    try {
      colorExtractor ??= new FastAverageColor()
      const result = await colorExtractor.getColorAsync(thumbnail, {
        algorithm: 'dominant',
        mode: 'speed',
        silent: true,
      })
      if (requestId === extractionRequestId && !result.error) coverColor.value = result.hex
    } catch (error) {
      if (requestId === extractionRequestId) {
        coverColor.value = null
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
