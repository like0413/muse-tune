import type { UnlistenFn } from '@tauri-apps/api/event'

import { reportBackgroundFailure } from '@/features/feedback/errors'

/** 一次「事件推送 + 初值读取」绑定所需的动作，由各能力的 client 或设置模块提供。 */
export interface EventStateBinding<T> {
  /** 读取当前值，用于补上订阅建立之前已经存在的状态。 */
  read: () => Promise<T>
  /** 订阅变更事件，返回取消订阅函数。 */
  subscribe: (handler: (value: T) => void) => Promise<UnlistenFn>
  /** 初始化失败时的日志说明。 */
  failureMessage: string
}

/**
 * 把「事件推送 + 初值读取」封装为竞态安全的响应式状态。
 *
 * 必须先订阅再读取：两步之间到达的事件比 `read()` 的返回值更新，读取返回时
 * 若已有事件到达就不能再覆盖，否则刚发布的新值会被旧值顶掉。组件卸载后到达的
 * 订阅与读取结果一律丢弃，避免已经离开的界面继续持有监听。
 */
export function useEventState<T>(binding: EventStateBinding<T>, initial: T): ShallowRef<T> {
  const value = shallowRef(initial)
  // 事件到达即自增：读取返回时版本未变才允许写入，版本已变说明 `read()` 拿到的是过期快照。
  let eventRevision = 0
  // 卸载后到达的订阅与读取结果一律丢弃，否则已经离场的界面会继续持有监听并写入无用状态。
  let disposed = false
  let unlisten: UnlistenFn | undefined

  async function initialize() {
    try {
      const stopListener = await binding.subscribe((next) => {
        eventRevision += 1
        value.value = next
      })
      if (disposed) {
        stopListener()
        return
      }
      unlisten = stopListener

      const revisionBeforeRead = eventRevision
      const initialValue = await binding.read()
      if (!disposed && eventRevision === revisionBeforeRead) value.value = initialValue
    } catch (error) {
      reportBackgroundFailure(binding.failureMessage, error)
    }
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })

  return value
}
