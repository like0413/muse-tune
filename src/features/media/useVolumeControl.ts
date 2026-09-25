import { listen } from '@tauri-apps/api/event'

import { reportRepeatedFailure } from '@/features/feedback/errors'
import { useEventState } from '@/features/ipc/useEventState'
import {
  DEFAULT_VOLUME_CONTROL_TARGET,
  getVolumeControlTarget,
  listenVolumeControlTargetChange,
  type VolumeControlTarget,
} from '@/features/settings/volume-control'

import {
  getCurrentMediaVolume,
  getSystemVolume,
  MEDIA_VOLUME_CHANGED_EVENT,
  setCurrentMediaVolume,
  setSystemVolume,
  SYSTEM_VOLUME_CHANGED_EVENT,
  toggleCurrentMediaMute,
  toggleSystemMute,
} from './client'
import type { MediaVolumeSnapshot } from './types'

type VolumeMutation =
  | { target: VolumeControlTarget; type: 'level'; level: number }
  | { target: VolumeControlTarget; type: 'toggle-mute' }

/** 订阅指定来源的原生音量事件。 */
async function listenVolumeChange(
  event: string,
  handler: (volume: MediaVolumeSnapshot | null) => void,
) {
  return listen<MediaVolumeSnapshot | null>(event, ({ payload }) => handler(payload))
}

/** 按设置路由当前播放器音量或 Windows 系统主音量，并合并连续写入。 */
export function useVolumeControl() {
  const target = useEventState(
    {
      read: getVolumeControlTarget,
      subscribe: listenVolumeControlTargetChange,
      failureMessage: '初始化音量控制对象失败',
    },
    DEFAULT_VOLUME_CONTROL_TARGET,
  )
  const applicationVolume = useEventState<MediaVolumeSnapshot | null>(
    {
      read: getCurrentMediaVolume,
      subscribe: (handler) => listenVolumeChange(MEDIA_VOLUME_CHANGED_EVENT, handler),
      failureMessage: '初始化当前播放器音量失败',
    },
    null,
  )
  const systemVolume = useEventState<MediaVolumeSnapshot | null>(
    {
      read: getSystemVolume,
      subscribe: (handler) => listenVolumeChange(SYSTEM_VOLUME_CHANGED_EVENT, handler),
      failureMessage: '初始化系统主音量失败',
    },
    null,
  )
  const volume = computed(() =>
    target.value === 'application' ? applicationVolume.value : systemVolume.value,
  )
  const pendingMutations: VolumeMutation[] = []
  let applying = false
  let disposed = false

  /** 替换指定控制对象的视觉快照。 */
  function updateSnapshot(
    controlTarget: VolumeControlTarget,
    snapshot: MediaVolumeSnapshot | null,
  ) {
    if (controlTarget === 'application') applicationVolume.value = snapshot
    else systemVolume.value = snapshot
  }

  /** 读取指定控制对象的原生真实状态。 */
  function readSnapshot(controlTarget: VolumeControlTarget) {
    return controlTarget === 'application' ? getCurrentMediaVolume() : getSystemVolume()
  }

  /** 提交指定控制对象的音量。 */
  function writeLevel(controlTarget: VolumeControlTarget, level: number) {
    return controlTarget === 'application' ? setCurrentMediaVolume(level) : setSystemVolume(level)
  }

  /** 切换指定控制对象的原生静音状态。 */
  function writeMuted(controlTarget: VolumeControlTarget) {
    return controlTarget === 'application' ? toggleCurrentMediaMute() : toggleSystemMute()
  }

  /** 立即更新视觉值，并只保留同一控制对象尚未发送的最后一次音量请求。 */
  function setLevel(level: number) {
    if (!volume.value) return
    const mutationTarget = target.value
    const normalized = Math.min(1, Math.max(0, level))
    updateSnapshot(mutationTarget, { level: normalized, muted: false })
    const lastMutation = pendingMutations.at(-1)
    if (lastMutation?.type === 'level' && lastMutation.target === mutationTarget) {
      lastMutation.level = normalized
    } else {
      pendingMutations.push({ target: mutationTarget, type: 'level', level: normalized })
    }
    if (!applying) void flushMutations()
  }

  /** 按用户操作顺序切换静音，同时保留变更发生时的控制对象。 */
  function toggleMuted() {
    if (!volume.value) return
    const mutationTarget = target.value
    updateSnapshot(mutationTarget, { ...volume.value, muted: !volume.value.muted })
    pendingMutations.push({ target: mutationTarget, type: 'toggle-mute' })
    if (!applying) void flushMutations()
  }

  /** 串行提交音量操作；失败时只刷新对应控制对象，避免覆盖另一个对象。 */
  async function flushMutations() {
    applying = true
    try {
      while (!disposed && pendingMutations.length > 0) {
        const mutation = pendingMutations.shift()
        if (!mutation) continue
        try {
          const snapshot =
            mutation.type === 'level'
              ? await writeLevel(mutation.target, mutation.level)
              : await writeMuted(mutation.target)
          updateSnapshot(mutation.target, snapshot)
        } catch (error) {
          reportRepeatedFailure('修改音量失败', error)
          try {
            updateSnapshot(mutation.target, await readSnapshot(mutation.target))
          } catch (refreshError) {
            reportRepeatedFailure('刷新音量失败', refreshError)
          }
        }
      }
    } finally {
      applying = false
    }
  }

  /** 按固定百分点调整当前设置所选择的音量对象。 */
  function adjustLevel(direction: 1 | -1, step: number = 0.02) {
    if (!volume.value) return
    setLevel(volume.value.level + direction * step)
  }

  onUnmounted(() => {
    disposed = true
    pendingMutations.length = 0
  })

  return {
    target: readonly(target),
    volume,
    setLevel,
    adjustLevel,
    toggleMuted,
  }
}
