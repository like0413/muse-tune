import type { UnlistenFn } from '@tauri-apps/api/event'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import {
  getMediaSessionSelectionPolicy,
  listenMediaSessionSelectionPolicyChange,
} from '@/features/settings/media-session'

import { setMediaSessionSelectionPolicy } from './client'
import type { MediaSessionSelectionPolicy } from './types'

/** 将前端持久化策略同步到持有全部 WinRT 会话的 Rust 服务。 */
export function useMediaSessionSelectionPolicy() {
  let unlisten: UnlistenFn | undefined
  let policyRevision = 0
  let disposed = false
  let applyQueue = Promise.resolve()

  /** 应用一次完整策略配置。 */
  function applyPolicy(policy: MediaSessionSelectionPolicy): Promise<void> {
    applyQueue = applyQueue.then(async () => {
      try {
        await setMediaSessionSelectionPolicy(policy)
      } catch (error) {
        reportBackgroundFailure('应用媒体会话选择策略失败', error)
      }
    })
    return applyQueue
  }

  /** 先监听设置变更，再读取持久化值，避免初始化期间漏掉更新。 */
  async function initialize() {
    try {
      const stopListener = await listenMediaSessionSelectionPolicyChange((policy) => {
        policyRevision += 1
        void applyPolicy(policy)
      })
      if (disposed) {
        stopListener()
        return
      }
      unlisten = stopListener
      const revisionBeforeRead = policyRevision
      const policy = await getMediaSessionSelectionPolicy()
      if (!disposed && policyRevision === revisionBeforeRead) await applyPolicy(policy)
    } catch (error) {
      reportBackgroundFailure('初始化媒体会话选择策略失败', error)
    }
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })
}
