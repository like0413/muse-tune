import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import {
  DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS,
  getTaskbarAudioSpectrumSettings,
  listenTaskbarAudioSpectrumSettingsChange,
} from '@/features/settings/audio-spectrum'

const MEDIA_SPECTRUM_CHANGED_EVENT = 'media://spectrum-changed'
const SOURCE_BAND_COUNT = 64

/** 订阅真实播放器频谱与显示设置，并按显隐状态启停原生采集。 */
export function useAudioSpectrum() {
  const settings = shallowRef({ ...DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS })
  const sourceBands = shallowRef<number[]>(createSilentFrame())
  let captureEnabled = false
  let disposed = false
  let unlistenFrame: UnlistenFn | undefined
  let unlistenSettings: UnlistenFn | undefined

  /** 仅在显隐状态实际变化时调用原生层，避免样式预览重复触发 IPC。 */
  async function synchronizeCapture(enabled: boolean) {
    if (captureEnabled === enabled) return
    captureEnabled = enabled
    try {
      await invoke('set_media_spectrum_enabled', { enabled })
    } catch (error) {
      console.error('切换播放器频谱采集失败', error)
    }
  }

  /** 更新配置，并让原生采集生命周期跟随显示开关。 */
  function updateSettings(next: typeof settings.value) {
    settings.value = next
    void synchronizeCapture(next.visible)
    if (!next.visible) sourceBands.value = createSilentFrame()
  }

  /** 先注册两个事件，再恢复配置，避免窗口加载期间漏掉变化。 */
  async function initialize() {
    try {
      const [stopFrameListener, stopSettingsListener] = await Promise.all([
        listen<number[]>(MEDIA_SPECTRUM_CHANGED_EVENT, ({ payload }) => {
          if (payload.length === SOURCE_BAND_COUNT) sourceBands.value = payload
        }),
        listenTaskbarAudioSpectrumSettingsChange(updateSettings),
      ])
      if (disposed) {
        stopFrameListener()
        stopSettingsListener()
        return
      }
      unlistenFrame = stopFrameListener
      unlistenSettings = stopSettingsListener
      updateSettings(await getTaskbarAudioSpectrumSettings())
    } catch (error) {
      console.error('初始化播放器频谱失败', error)
    }
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlistenFrame?.()
    unlistenSettings?.()
    if (captureEnabled) void synchronizeCapture(false)
  })

  return {
    settings: readonly(settings),
    sourceBands: readonly(sourceBands),
  }
}

/** 创建与原生频带数量一致的静默帧。 */
function createSilentFrame(): number[] {
  return Array.from({ length: SOURCE_BAND_COUNT }, () => 0)
}
