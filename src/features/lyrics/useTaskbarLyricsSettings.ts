import type { UnlistenFn } from '@tauri-apps/api/event'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_LYRICS_SETTINGS,
  getTaskbarLyricsSettings,
  listenTaskbarLyricsSettingsChange,
} from '@/features/settings/lyrics'

/** 为单个任务栏窗口恢复并订阅歌词显示配置。 */
export function useTaskbarLyricsSettings() {
  const settings = shallowRef({ ...DEFAULT_TASKBAR_LYRICS_SETTINGS })
  let receivedEvent = false
  let disposed = false
  let unlisten: UnlistenFn | undefined

  /** 先监听后读取，避免窗口初始化期间覆盖较新的配置事件。 */
  async function initialize() {
    try {
      const stopListener = await listenTaskbarLyricsSettingsChange((next) => {
        receivedEvent = true
        settings.value = next
      })
      if (disposed) {
        stopListener()
        return
      }
      unlisten = stopListener
      const initial = await getTaskbarLyricsSettings()
      if (!disposed && !receivedEvent) settings.value = initial
    } catch (error) {
      reportBackgroundFailure('初始化歌词显示配置失败', error)
    }
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })

  return { settings: readonly(settings) }
}
