import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'
import { onMounted, onUnmounted, readonly, shallowRef } from 'vue'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import { hideVolumePopup } from '@/features/taskbar/client'

import {
  VOLUME_POPUP_CLOSE_EVENT,
  VOLUME_POPUP_HOVER_CHANGED_EVENT,
  VOLUME_POPUP_OPEN_EVENT,
  VOLUME_POPUP_TRANSITION_MS,
  type VolumePopupOpenPayload,
  type VolumePopupOwnerPayload,
} from './volume-popup'

/** 管理音量悬浮窗的事件协议、进出场状态和原生窗口生命周期。 */
export function useVolumePopupLifecycle() {
  const ownerLabel = shallowRef('')
  const themeColor = shallowRef('#1677ff')
  const generation = shallowRef(0)
  const entered = shallowRef(false)
  let hideTimer: ReturnType<typeof setTimeout> | undefined
  let animationFrame: number | undefined
  let unlistenOpen: UnlistenFn | undefined
  let unlistenClose: UnlistenFn | undefined
  let disposed = false

  /** 清除尚未执行的动画调度。 */
  function clearAnimationTimers() {
    if (hideTimer) clearTimeout(hideTimer)
    if (animationFrame !== undefined) cancelAnimationFrame(animationFrame)
    hideTimer = undefined
    animationFrame = undefined
  }

  /** 重置离场状态并在下一帧播放等时长进入动画。 */
  function open(payload: VolumePopupOpenPayload) {
    clearAnimationTimers()
    ownerLabel.value = payload.ownerLabel
    themeColor.value = payload.themeColor
    generation.value = payload.generation
    entered.value = false
    animationFrame = requestAnimationFrame(() => {
      entered.value = true
      animationFrame = undefined
    })
  }

  /** 播放离场动画，结束后再隐藏原生窗。 */
  function close(payload: VolumePopupOwnerPayload) {
    if (payload.ownerLabel !== ownerLabel.value) return
    clearAnimationTimers()
    entered.value = false
    const closingGeneration = generation.value
    hideTimer = setTimeout(() => {
      hideTimer = undefined
      void hideVolumePopup(closingGeneration).catch((error) => {
        reportBackgroundFailure('隐藏音量悬浮窗失败', error)
      })
    }, VOLUME_POPUP_TRANSITION_MS)
  }

  /** 将整个悬浮柱纳入同一 hover 区域。 */
  function publishHover(hovered: boolean) {
    if (!ownerLabel.value) return
    void emit(VOLUME_POPUP_HOVER_CHANGED_EVENT, {
      ownerLabel: ownerLabel.value,
      hovered,
    }).catch((error) => {
      reportBackgroundFailure('同步音量悬浮窗悬停状态失败', error)
    })
  }

  /** 注册弹窗事件，并处理初始化期间页面提前卸载的竞态。 */
  async function initialize() {
    const listeners: UnlistenFn[] = []
    try {
      listeners.push(
        await listen<VolumePopupOpenPayload>(VOLUME_POPUP_OPEN_EVENT, ({ payload }) =>
          open(payload),
        ),
      )
      listeners.push(
        await listen<VolumePopupOwnerPayload>(VOLUME_POPUP_CLOSE_EVENT, ({ payload }) =>
          close(payload),
        ),
      )
      if (disposed) {
        listeners.forEach((stopListener) => stopListener())
        return
      }
      ;[unlistenOpen, unlistenClose] = listeners
    } catch (error) {
      listeners.forEach((stopListener) => stopListener())
      reportBackgroundFailure('初始化音量悬浮窗事件失败', error)
    }
  }

  onMounted(() => void initialize())
  onUnmounted(() => {
    disposed = true
    clearAnimationTimers()
    unlistenOpen?.()
    unlistenClose?.()
  })

  return {
    themeColor: readonly(themeColor),
    entered: readonly(entered),
    publishHover,
  }
}
