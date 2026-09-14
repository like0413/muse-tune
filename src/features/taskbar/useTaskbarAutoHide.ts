import type { UnlistenFn } from '@tauri-apps/api/event'
import type { Ref } from 'vue'

import type { MediaSessionSnapshot } from '@/features/media/types'
import {
  DEFAULT_TASKBAR_AUTO_HIDE,
  getTaskbarAutoHide,
  listenTaskbarAutoHideChange,
  type TaskbarAutoHide,
} from '@/features/settings/bar-visibility'

import { setTaskbarContentVisibility } from './client'

/** 根据媒体状态与用户偏好，驱动原生 bar 窗口的可见性门控。 */
export function useTaskbarAutoHide(session: Readonly<Ref<MediaSessionSnapshot | null>>) {
  const preference = shallowRef<TaskbarAutoHide>({ ...DEFAULT_TASKBAR_AUTO_HIDE })
  let preferenceRevision = 0
  let disposed = false
  let unlisten: UnlistenFn | undefined
  let appliedVisibility: boolean | undefined

  const visible = computed(() => {
    if (!session.value) return !preference.value.whenNoMediaSession
    if (session.value.playback.status === 'paused') return !preference.value.whenPaused
    return true
  })

  /** 仅在计算结果变化时跨 IPC 更新原生窗口。 */
  async function applyVisibility(value: boolean) {
    if (appliedVisibility === value) return
    appliedVisibility = value
    try {
      await setTaskbarContentVisibility(value)
    } catch (error) {
      appliedVisibility = undefined
      console.error('更新任务栏播放器可见性失败', error)
    }
  }

  /** 先监听再读取，避免初始化期间丢失设置变更。 */
  async function initialize() {
    try {
      const stopListener = await listenTaskbarAutoHideChange((value) => {
        preferenceRevision += 1
        preference.value = value
      })
      if (disposed) {
        stopListener()
        return
      }
      unlisten = stopListener
      const revisionBeforeRead = preferenceRevision
      const savedPreference = await getTaskbarAutoHide()
      if (!disposed && preferenceRevision === revisionBeforeRead) preference.value = savedPreference
    } catch (error) {
      console.error('初始化任务栏播放器自动隐藏配置失败', error)
    }
  }

  watch(visible, (value) => void applyVisibility(value), { immediate: true })
  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })
}
