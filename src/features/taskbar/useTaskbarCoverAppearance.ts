import type { UnlistenFn } from '@tauri-apps/api/event'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_COVER_APPEARANCE,
  getTaskbarCoverAppearance,
  listenTaskbarCoverAppearanceChange,
  type TaskbarCoverAppearance,
} from '@/features/settings/cover'

/** 同步封面显示配置，由任务栏布局统一传给封面组件和两个占位锚点。 */
export function useTaskbarCoverAppearance() {
  const appearance = shallowRef<TaskbarCoverAppearance>({ ...DEFAULT_TASKBAR_COVER_APPEARANCE })
  let eventRevision = 0
  let disposed = false
  let unlisten: UnlistenFn | undefined

  /** 先监听事件再读取存储，避免初始化期间覆盖设置窗口刚发布的新值。 */
  async function initialize() {
    try {
      const stopListener = await listenTaskbarCoverAppearanceChange((value) => {
        eventRevision += 1
        appearance.value = value
      })
      if (disposed) {
        stopListener()
        return
      }
      unlisten = stopListener

      const revisionBeforeRead = eventRevision
      const saved = await getTaskbarCoverAppearance()
      if (!disposed && eventRevision === revisionBeforeRead) appearance.value = saved
    } catch (error) {
      reportBackgroundFailure('初始化封面配置失败', error)
    }
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })

  return {
    appearance: readonly(appearance),
  }
}
