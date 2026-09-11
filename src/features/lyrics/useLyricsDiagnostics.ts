import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import type { LyricsDiagnostics } from './types'

const LYRICS_DIAGNOSTICS_CHANGED_EVENT = 'lyrics://diagnostics-changed'

/** 按歌词事件刷新只读诊断，设置页关闭后立即释放监听。 */
export function useLyricsDiagnostics() {
  const diagnostics = shallowRef<LyricsDiagnostics | null>(null)
  let disposed = false
  let requestId = 0
  let unlisten: UnlistenFn | undefined

  /** 只提交最后一次请求，避免连续歌词事件造成旧诊断覆盖新状态。 */
  async function refresh() {
    const currentRequest = ++requestId
    try {
      const next = await invoke<LyricsDiagnostics>('get_lyrics_diagnostics')
      if (!disposed && currentRequest === requestId) diagnostics.value = next
    } catch (error) {
      console.error('读取歌词诊断失败', error)
    }
  }

  /** 先订阅再读取快照，覆盖设置窗口初始化期间的状态变化。 */
  async function initialize() {
    try {
      const stopListener = await listen(LYRICS_DIAGNOSTICS_CHANGED_EVENT, () => void refresh())
      if (disposed) {
        stopListener()
        return
      }
      unlisten = stopListener
      await refresh()
    } catch (error) {
      console.error('初始化歌词诊断失败', error)
    }
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })

  return { diagnostics: readonly(diagnostics), refresh }
}
