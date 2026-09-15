import type { UnlistenFn } from '@tauri-apps/api/event'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import {
  getTaskbarBackgroundTransparency,
  listenTaskbarBackgroundTransparencyChange,
} from '@/features/settings/background-transparency'
import {
  getTaskbarWidth,
  listenTaskbarWidthChange,
  TASKBAR_WIDTH_PRESETS,
} from '@/features/settings/bar-width'
import {
  DEFAULT_TASKBAR_ELEMENT_ORDER,
  getTaskbarElementOrder,
  listenTaskbarElementOrderChange,
  type TaskbarElement,
} from '@/features/settings/element-order'
import {
  DEFAULT_TASKBAR_PROGRESS_POSITION,
  DEFAULT_TASKBAR_PROGRESS_STYLE,
  getTaskbarProgressPosition,
  getTaskbarProgressStyle,
  listenTaskbarProgressPositionChange,
  listenTaskbarProgressStyleChange,
  type TaskbarProgressPosition,
  type TaskbarProgressStyle,
} from '@/features/settings/progress-style'

interface SettingBinding<T> {
  load: () => Promise<T>
  listen: (handler: (value: T) => void) => Promise<UnlistenFn>
  apply: (value: T) => void
  failureMessage: string
}

/** 同步任务栏窗口使用的设置，并统一处理异步监听注册与卸载竞态。 */
export function useTaskbarViewSettings() {
  const backgroundTransparency = shallowRef(0)
  const progressStyle = shallowRef<TaskbarProgressStyle>(DEFAULT_TASKBAR_PROGRESS_STYLE)
  const progressPosition = shallowRef<TaskbarProgressPosition>(DEFAULT_TASKBAR_PROGRESS_POSITION)
  const elementOrder = shallowRef<TaskbarElement[]>([...DEFAULT_TASKBAR_ELEMENT_ORDER])
  const taskbarWidth = shallowRef<number>(TASKBAR_WIDTH_PRESETS.wide)
  const unlisteners: UnlistenFn[] = []
  let disposed = false

  /** 先订阅再读取；事件先到达时，过期读取不得覆盖较新的设置。 */
  async function bindSetting<T>(binding: SettingBinding<T>): Promise<void> {
    let eventRevision = 0
    try {
      const unlisten = await binding.listen((value) => {
        eventRevision += 1
        binding.apply(value)
      })
      if (disposed) {
        unlisten()
        return
      }
      unlisteners.push(unlisten)

      const revisionBeforeRead = eventRevision
      const saved = await binding.load()
      if (!disposed && eventRevision === revisionBeforeRead) {
        binding.apply(saved)
      }
    } catch (error) {
      reportBackgroundFailure(binding.failureMessage, error)
    }
  }

  /** 并行初始化彼此独立的设置绑定，单项失败不阻断其他设置。 */
  function initialize() {
    void Promise.all([
      bindSetting({
        load: getTaskbarBackgroundTransparency,
        listen: listenTaskbarBackgroundTransparencyChange,
        apply: (value) => (backgroundTransparency.value = value),
        failureMessage: '初始化任务栏背景透明度失败',
      }),
      bindSetting({
        load: getTaskbarProgressStyle,
        listen: listenTaskbarProgressStyleChange,
        apply: (value) => (progressStyle.value = value),
        failureMessage: '初始化播放进度样式失败',
      }),
      bindSetting({
        load: getTaskbarProgressPosition,
        listen: listenTaskbarProgressPositionChange,
        apply: (value) => (progressPosition.value = value),
        failureMessage: '初始化播放进度位置失败',
      }),
      bindSetting({
        load: getTaskbarElementOrder,
        listen: listenTaskbarElementOrderChange,
        apply: (value) => (elementOrder.value = value),
        failureMessage: '初始化任务栏区块顺序失败',
      }),
      bindSetting({
        load: getTaskbarWidth,
        listen: listenTaskbarWidthChange,
        apply: (value) => (taskbarWidth.value = value),
        failureMessage: '初始化任务栏宽度状态失败',
      }),
    ])
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlisteners.splice(0).forEach((unlisten) => unlisten())
  })

  return {
    backgroundTransparency: readonly(backgroundTransparency),
    progressStyle: readonly(progressStyle),
    progressPosition: readonly(progressPosition),
    elementOrder: readonly(elementOrder),
    taskbarWidth: readonly(taskbarWidth),
  }
}
