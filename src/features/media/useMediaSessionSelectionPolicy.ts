import { reportBackgroundFailure } from '@/features/feedback/errors'
import { useEventState } from '@/features/ipc/useEventState'
import {
  DEFAULT_MEDIA_SESSION_SELECTION_POLICY,
  getMediaSessionSelectionPolicy,
  listenMediaSessionSelectionPolicyChange,
} from '@/features/settings/media-session'

import { setMediaSessionSelectionPolicy } from './client'
import type { MediaSessionSelectionPolicy } from './types'

/** 将前端持久化策略同步到持有全部 WinRT 会话的 Rust 服务。 */
export function useMediaSessionSelectionPolicy() {
  // 原生侧按到达顺序应用策略，乱序下发会让当前会话指向错误的播放器，因此全部串行排队。
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

  const policy = useEventState(
    {
      read: getMediaSessionSelectionPolicy,
      subscribe: listenMediaSessionSelectionPolicyChange,
      failureMessage: '初始化媒体会话选择策略失败',
    },
    DEFAULT_MEDIA_SESSION_SELECTION_POLICY,
  )
  // 首次下发由初值读取触发，不额外提前下发默认策略；随后的变更逐次补发。
  watch(policy, (next) => void applyPolicy(next))
}
