import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import type { MediaVolumeSnapshot } from './types'

const MEDIA_VOLUME_CHANGED_EVENT = 'media://volume-changed'

type VolumeMutation = { type: 'level'; level: number } | { type: 'toggle-mute' }

/** 订阅当前播放器的 Windows 单应用音量，并合并连续写入避免 IPC 排队。 */
export function useApplicationVolume() {
  const volume = shallowRef<MediaVolumeSnapshot | null>(null)
  let eventVersion = 0
  let disposed = false
  let applying = false
  const pendingMutations: VolumeMutation[] = []
  let unlistenVolume: UnlistenFn | undefined

  /** 先监听系统回调，再读取当前值，避免初始化间隙丢事件。 */
  async function initialize() {
    try {
      const stopListener = await listen<MediaVolumeSnapshot | null>(
        MEDIA_VOLUME_CHANGED_EVENT,
        ({ payload }) => {
          eventVersion += 1
          volume.value = payload
        },
      )
      if (disposed) {
        stopListener()
        return
      }
      unlistenVolume = stopListener
      // 只让读取期间的新事件覆盖快照，订阅建立期间的旧空值不能阻止初始化。
      const versionBeforeRead = eventVersion
      const initial = await invoke<MediaVolumeSnapshot | null>('get_current_media_volume')
      if (!disposed && eventVersion === versionBeforeRead) volume.value = initial
    } catch (error) {
      console.error('初始化播放器应用音量失败', error)
    }
  }

  /** 立即更新视觉值，并始终只保留尚未发送的最后一次音量请求。 */
  function setLevel(level: number) {
    if (!volume.value) return
    const normalized = Math.min(1, Math.max(0, level))
    volume.value = { level: normalized, muted: false }
    const lastMutation = pendingMutations.at(-1)
    if (lastMutation?.type === 'level') lastMutation.level = normalized
    else pendingMutations.push({ type: 'level', level: normalized })
    if (!applying) void flushMutations()
  }

  /** 按用户操作顺序切换静音，同时继续合并相邻的音量写入。 */
  function toggleMuted() {
    if (!volume.value) return
    volume.value = { ...volume.value, muted: !volume.value.muted }
    pendingMutations.push({ type: 'toggle-mute' })
    if (!applying) void flushMutations()
  }

  /** 串行提交音量与静音操作，播放器会话消失时恢复后端真实状态。 */
  async function flushMutations() {
    applying = true
    try {
      while (!disposed && pendingMutations.length > 0) {
        const mutation = pendingMutations.shift()
        if (!mutation) continue
        try {
          volume.value =
            mutation.type === 'level'
              ? await invoke<MediaVolumeSnapshot>('set_current_media_volume', {
                  level: mutation.level,
                })
              : await invoke<MediaVolumeSnapshot>('toggle_current_media_mute')
        } catch (error) {
          console.error('修改播放器应用音量失败', error)
          try {
            volume.value = await invoke<MediaVolumeSnapshot | null>('get_current_media_volume')
          } catch (refreshError) {
            console.error('刷新播放器应用音量失败', refreshError)
          }
        }
      }
    } finally {
      applying = false
    }
  }

  /** 按固定百分点调整音量，供按钮与悬浮柱滚轮共用。 */
  function adjustLevel(direction: 1 | -1, step = 0.02) {
    if (!volume.value) return
    setLevel(volume.value.level + direction * step)
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    pendingMutations.length = 0
    unlistenVolume?.()
  })

  return {
    volume: readonly(volume),
    setLevel,
    adjustLevel,
    toggleMuted,
  }
}
