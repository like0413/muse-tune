import { useEventState } from '@/features/ipc/useEventState'
import {
  DEFAULT_TASKBAR_COVER_APPEARANCE,
  getTaskbarCoverAppearance,
  listenTaskbarCoverAppearanceChange,
} from '@/features/settings/cover'

/** 同步封面显示配置，由任务栏布局统一传给封面组件和两个占位锚点。 */
export function useTaskbarCoverAppearance() {
  const appearance = useEventState(
    {
      read: getTaskbarCoverAppearance,
      subscribe: listenTaskbarCoverAppearanceChange,
      failureMessage: '初始化封面配置失败',
    },
    { ...DEFAULT_TASKBAR_COVER_APPEARANCE },
  )

  return {
    appearance: readonly(appearance),
  }
}
