import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import {
  controlMediaSession,
  getCurrentMediaSession,
  MEDIA_SESSION_CHANGED_EVENT,
  MEDIA_TIMELINE_CHANGED_EVENT,
} from './client'
import type { MediaControlAction, MediaSessionSnapshot, MediaTimeline } from './types'

/** 同步 Windows 当前媒体会话，并提供串行基础播放控制。 */
export function useMediaSession() {
  const session = shallowRef<MediaSessionSnapshot | null>(null)
  const timeline = shallowRef<MediaTimeline | null>(null)
  const controlPending = shallowRef(false)
  let receivedEvent = false
  let disposed = false
  let unlistenSession: UnlistenFn | undefined
  let unlistenTimeline: UnlistenFn | undefined

  /** 先建立事件监听，再读取缓存，避免页面加载期间漏掉会话切换。 */
  async function initialize() {
    try {
      const [stopSessionListener, stopTimelineListener] = await Promise.all([
        listen<MediaSessionSnapshot | null>(MEDIA_SESSION_CHANGED_EVENT, ({ payload }) => {
          receivedEvent = true
          session.value = payload
          timeline.value = payload?.timeline ?? null
        }),
        listen<MediaTimeline | null>(MEDIA_TIMELINE_CHANGED_EVENT, ({ payload }) => {
          timeline.value = payload
        }),
      ])
      if (disposed) {
        stopSessionListener()
        stopTimelineListener()
        return
      }
      unlistenSession = stopSessionListener
      unlistenTimeline = stopTimelineListener
      const initial = await getCurrentMediaSession()
      if (!disposed && !receivedEvent) {
        session.value = initial
        timeline.value = initial?.timeline ?? null
      }
    } catch (error) {
      console.error('初始化 Windows 媒体会话失败', error)
    }
  }

  /** 向当前会话发送控制请求，播放器拒绝时保留现有状态等待后续系统事件。 */
  async function control(action: MediaControlAction) {
    if (controlPending.value) return
    controlPending.value = true
    try {
      const accepted = await controlMediaSession(action)
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
    unlistenSession?.()
    unlistenTimeline?.()
  })

  return {
    session: readonly(session),
    timeline: readonly(timeline),
    controlPending: readonly(controlPending),
    control,
  }
}
