import type { UnlistenFn } from '@tauri-apps/api/event'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import { applyApplicationLocale } from '@/features/i18n'
import { listenApplicationLocaleChange } from '@/features/i18n/settings'

/** 在当前 WebView 生命周期内同步其他窗口发布的语言变化。 */
export function useApplicationLocaleSync() {
  let unlisten: UnlistenFn | undefined
  let disposed = false

  /** 注册语言事件，并补偿监听初始化期间发生的卸载。 */
  async function initialize() {
    try {
      const stopListener = await listenApplicationLocaleChange(applyApplicationLocale)
      if (disposed) stopListener()
      else unlisten = stopListener
    } catch (error) {
      reportBackgroundFailure('初始化界面语言同步失败', error)
    }
  }

  onMounted(() => void initialize())
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })
}
