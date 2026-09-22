import { useEventState } from '@/features/ipc/useEventState'
import {
  DEFAULT_TASKBAR_BACKGROUND_STYLE,
  getTaskbarBackgroundStyle,
  listenTaskbarBackgroundStyleChange,
} from '@/features/settings/background-style'
import {
  getTaskbarBackgroundTransparency,
  listenTaskbarBackgroundTransparencyChange,
} from '@/features/settings/background-transparency'
import { DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY } from '@/features/settings/defaults'
import {
  cloneTaskbarElementOrder,
  DEFAULT_TASKBAR_ELEMENT_ORDER,
  getTaskbarElementOrder,
  listenTaskbarElementOrderChange,
} from '@/features/settings/element-order'
import {
  DEFAULT_TASKBAR_PROGRESS_POSITION,
  DEFAULT_TASKBAR_PROGRESS_STYLE,
  DEFAULT_TASKBAR_PROGRESS_VISIBLE,
  getTaskbarProgressPosition,
  getTaskbarProgressStyle,
  getTaskbarProgressVisible,
  listenTaskbarProgressPositionChange,
  listenTaskbarProgressStyleChange,
  listenTaskbarProgressVisibleChange,
} from '@/features/settings/progress-style'

/** 同步任务栏窗口使用的设置，每一项独立订阅，单项失败不阻断其他设置。 */
export function useTaskbarViewSettings() {
  const backgroundTransparency = useEventState(
    {
      read: getTaskbarBackgroundTransparency,
      subscribe: listenTaskbarBackgroundTransparencyChange,
      failureMessage: '初始化任务栏背景透明度失败',
    },
    DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY,
  )
  const backgroundStyle = useEventState(
    {
      read: getTaskbarBackgroundStyle,
      subscribe: listenTaskbarBackgroundStyleChange,
      failureMessage: '初始化任务栏背景样式失败',
    },
    DEFAULT_TASKBAR_BACKGROUND_STYLE,
  )
  const progressStyle = useEventState(
    {
      read: getTaskbarProgressStyle,
      subscribe: listenTaskbarProgressStyleChange,
      failureMessage: '初始化播放进度样式失败',
    },
    DEFAULT_TASKBAR_PROGRESS_STYLE,
  )
  const progressVisible = useEventState(
    {
      read: getTaskbarProgressVisible,
      subscribe: listenTaskbarProgressVisibleChange,
      failureMessage: '初始化进度条显隐失败',
    },
    DEFAULT_TASKBAR_PROGRESS_VISIBLE,
  )
  const progressPosition = useEventState(
    {
      read: getTaskbarProgressPosition,
      subscribe: listenTaskbarProgressPositionChange,
      failureMessage: '初始化播放进度位置失败',
    },
    DEFAULT_TASKBAR_PROGRESS_POSITION,
  )
  const elementOrder = useEventState(
    {
      read: getTaskbarElementOrder,
      subscribe: listenTaskbarElementOrderChange,
      failureMessage: '初始化任务栏区块顺序失败',
    },
    cloneTaskbarElementOrder(DEFAULT_TASKBAR_ELEMENT_ORDER),
  )

  return {
    backgroundStyle: readonly(backgroundStyle),
    backgroundTransparency: readonly(backgroundTransparency),
    progressStyle: readonly(progressStyle),
    progressVisible: readonly(progressVisible),
    progressPosition: readonly(progressPosition),
    elementOrder: readonly(elementOrder),
  }
}
