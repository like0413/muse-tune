import { reportBackgroundFailure } from '@/features/feedback/errors'
import { useEventState } from '@/features/ipc/useEventState'
import type { MediaSessionSnapshot } from '@/features/media/types'
import {
  DEFAULT_TASKBAR_AUTO_HIDE,
  getTaskbarAutoHide,
  listenTaskbarAutoHideChange,
} from '@/features/settings/bar-visibility'

import { setTaskbarContentVisibility } from './client'

/** 根据媒体状态与用户偏好，驱动原生 bar 窗口的可见性门控。 */
export function useTaskbarAutoHide(session: Readonly<Ref<MediaSessionSnapshot | null>>) {
  const preference = useEventState(
    {
      read: getTaskbarAutoHide,
      subscribe: listenTaskbarAutoHideChange,
      failureMessage: '初始化任务栏播放器自动隐藏配置失败',
    },
    { ...DEFAULT_TASKBAR_AUTO_HIDE },
  )
  let appliedVisibility: boolean | undefined
  let desiredVisibility: boolean | undefined
  let applyingVisibility = false

  // 只有 paused 有独立偏好；其余任何状态（含 stopped / closed / changing / unknown）都直接显示，
  // 新增状态默认可见，若要单独控制必须在此加分支。
  const visible = computed(() => {
    if (!session.value) return !preference.value.whenNoMediaSession
    if (session.value.playback.status === 'paused') return !preference.value.whenPaused
    return true
  })

  /**
   * 串行提交可见性，避免启动时“无会话”和随后恢复出的媒体会话并发写入，
   * 导致较早的隐藏请求反而最后抵达原生层。
   */
  async function flushVisibility() {
    if (applyingVisibility) return
    applyingVisibility = true
    while (desiredVisibility !== undefined && appliedVisibility !== desiredVisibility) {
      const requestedVisibility = desiredVisibility
      try {
        await setTaskbarContentVisibility(requestedVisibility)
        appliedVisibility = requestedVisibility
      } catch (error) {
        reportBackgroundFailure('更新任务栏播放器可见性失败', error)
        if (desiredVisibility === requestedVisibility) break
      }
    }
    applyingVisibility = false
  }

  /** 记录最新目标；进行中的写入结束后会继续追平最终状态。 */
  function applyVisibility(value: boolean) {
    desiredVisibility = value
    void flushVisibility()
  }

  watch(visible, applyVisibility, { immediate: true })

  return {
    /** 原生 bar 隐藏期间供高频视觉组件停止工作。 */
    visible: readonly(visible),
  }
}
