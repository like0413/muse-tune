import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import type { MediaControlAction, MediaSessionSnapshot } from './types'

const MEDIA_SESSION_CHANGED_EVENT = 'media://session-changed'

/** 同步 Windows 当前媒体会话，并提供串行基础播放控制。 */
export function useMediaSession() {
  const session = shallowRef<MediaSessionSnapshot | null>(null)
  const controlPending = shallowRef(false)
  let receivedEvent = false
  let disposed = false
  let unlisten: UnlistenFn | undefined

  /** 先建立事件监听，再读取缓存，避免页面加载期间漏掉会话切换。 */
  async function initialize() {
    try {
      const stopListener = await listen<MediaSessionSnapshot | null>(
        MEDIA_SESSION_CHANGED_EVENT,
        ({ payload }) => {
          receivedEvent = true
          session.value = payload
        },
      )
      if (disposed) {
        stopListener()
        return
      }
      unlisten = stopListener
      const initial = await invoke<MediaSessionSnapshot | null>('get_current_media_session')
      if (!disposed && !receivedEvent) session.value = initial
    } catch (error) {
      console.error('初始化 Windows 媒体会话失败', error)
    }
  }

  /** 向当前会话发送控制请求，播放器拒绝时保留现有状态等待后续系统事件。 */
  async function control(action: MediaControlAction) {
    if (controlPending.value) return
    controlPending.value = true
    try {
      const accepted = await invoke<boolean>('control_media_session', { action })
      if (!accepted) console.warn('当前播放器拒绝了媒体控制请求', action)
    } catch (error) {
      console.error('控制 Windows 媒体会话失败', error)
    } finally {
      controlPending.value = false
    }
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })

  return {
    session: readonly(session),
    controlPending: readonly(controlPending),
    control,
  }
}
