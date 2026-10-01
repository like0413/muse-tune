import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS,
  type TaskbarAudioSpectrumSettings,
} from '@/features/settings/audio-spectrum'

import { MEDIA_SPECTRUM_CHANGED_EVENT, setMediaSpectrumEnabled } from './client'
import { MEDIA_SPECTRUM_SOURCE_BAND_COUNT } from './spectrum'

/** 订阅真实播放器频谱与显示设置，并按显隐状态启停原生采集。 */
export function useAudioSpectrum(
  settings: Readonly<Ref<TaskbarAudioSpectrumSettings>> = shallowRef({
    ...DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS,
  }),
) {
  const sourceBands = shallowRef<readonly number[]>(createSilentFrame())
  let captureEnabled = false
  let disposed = false
  let unlistenFrame: UnlistenFn | undefined

  /**
   * 仅在显隐状态实际变化时调用原生层，避免样式预览重复触发 IPC。
   * `force` 用于帧率变更：显隐未变也必须把新节奏重新下发给原生采集。
   */
  async function synchronizeCapture(enabled: boolean, force = false) {
    if (!force && captureEnabled === enabled) return
    captureEnabled = enabled
    try {
      await setMediaSpectrumEnabled(enabled, settings.value.frameRate)
    } catch (error) {
      reportBackgroundFailure('切换播放器频谱采集失败', error)
    }
  }

  /** 只在组件实际挂载时订阅频谱帧并启动原生采集。 */
  async function initialize() {
    try {
      const stopFrameListener = await listen<number[]>(
        MEDIA_SPECTRUM_CHANGED_EVENT,
        ({ payload }) => {
          if (payload.length === MEDIA_SPECTRUM_SOURCE_BAND_COUNT) sourceBands.value = payload
        },
      )
      if (disposed) {
        stopFrameListener()
        return
      }
      unlistenFrame = stopFrameListener
      await synchronizeCapture(settings.value.visible)
    } catch (error) {
      reportBackgroundFailure('初始化播放器频谱失败', error)
    }
  }

  onMounted(initialize)
  watch(
    () => settings.value.frameRate,
    () => {
      if (captureEnabled) void synchronizeCapture(true, true)
    },
  )
  onUnmounted(() => {
    disposed = true
    unlistenFrame?.()
    if (captureEnabled) void synchronizeCapture(false)
  })

  return {
    settings: readonly(settings),
    // 频谱帧整体替换，数组只读由类型约束；避免每帧频带读取经过深层代理。
    sourceBands: shallowReadonly(sourceBands),
  }
}

/** 创建与原生频带数量一致的静默帧。 */
function createSilentFrame(): number[] {
  return Array.from({ length: MEDIA_SPECTRUM_SOURCE_BAND_COUNT }, () => 0)
}
