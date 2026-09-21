import { useThrottleFn } from '@vueuse/core'

import { notifySettingSaveFailed, reportBackgroundFailure } from '@/features/feedback/errors'
import {
  applyTaskbarTrackInfoScrolling,
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
  DEFAULT_TASKBAR_TRACK_INFO_SCROLLING,
  DEFAULT_TASKBAR_TRACK_INFO_VISIBLE,
  getTaskbarTrackInfoAlignment,
  getTaskbarTrackInfoScrolling,
  getTaskbarTrackInfoVisible,
  isTaskbarTrackInfoAlignment,
  isTaskbarTrackInfoScrollMode,
  normalizeTaskbarTrackInfoScrollSpeed,
  setTaskbarTrackInfoAlignment,
  setTaskbarTrackInfoScrolling,
  setTaskbarTrackInfoVisible,
  type TaskbarTrackInfoAlignment,
  type TaskbarTrackInfoScrolling,
} from '@/features/settings/track-info'

const SCROLL_PREVIEW_INTERVAL_MS = 50

/** 管理歌曲信息设置的读取、提交、预览与失败回滚。 */
export function useTrackInfoSetting() {
  const { t } = useI18n({ useScope: 'global' })
  const selectedAlignment = shallowRef<TaskbarTrackInfoAlignment>(
    DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
  )
  const committedAlignment = shallowRef<TaskbarTrackInfoAlignment>(
    DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
  )
  const alignmentSaving = shallowRef(false)
  const selectedScrolling = shallowRef<TaskbarTrackInfoScrolling>({
    ...DEFAULT_TASKBAR_TRACK_INFO_SCROLLING,
  })
  const committedScrolling = shallowRef<TaskbarTrackInfoScrolling>({
    ...DEFAULT_TASKBAR_TRACK_INFO_SCROLLING,
  })
  const scrollingSaving = shallowRef(false)
  const selectedVisible = shallowRef(DEFAULT_TASKBAR_TRACK_INFO_VISIBLE)
  const committedVisible = shallowRef(DEFAULT_TASKBAR_TRACK_INFO_VISIBLE)
  const visibilitySaving = shallowRef(false)

  /** 恢复已保存的歌曲信息配置。 */
  async function loadSettings() {
    try {
      const [visible, alignment, scrolling] = await Promise.all([
        getTaskbarTrackInfoVisible(),
        getTaskbarTrackInfoAlignment(),
        getTaskbarTrackInfoScrolling(),
      ])
      selectedVisible.value = visible
      committedVisible.value = visible
      selectedAlignment.value = alignment
      committedAlignment.value = alignment
      selectedScrolling.value = scrolling
      committedScrolling.value = { ...scrolling }
    } catch (error) {
      reportBackgroundFailure('读取歌曲信息配置失败', error)
    }
  }

  /** 保存歌曲信息整体显隐，失败时恢复最近成功值。 */
  async function updateVisible(visible: boolean) {
    if (visibilitySaving.value || visible === selectedVisible.value) return
    selectedVisible.value = visible
    visibilitySaving.value = true
    try {
      await setTaskbarTrackInfoVisible(visible)
      committedVisible.value = visible
    } catch (error) {
      selectedVisible.value = committedVisible.value
      notifySettingSaveFailed(t('settings.taskbar.trackInfo.visible'), error)
    } finally {
      visibilitySaving.value = false
    }
  }

  /** 保存歌曲信息对齐方式，失败时恢复最近成功值。 */
  async function selectAlignment(value: unknown) {
    if (
      alignmentSaving.value ||
      !isTaskbarTrackInfoAlignment(value) ||
      value === selectedAlignment.value
    ) {
      return
    }
    selectedAlignment.value = value
    alignmentSaving.value = true
    try {
      await setTaskbarTrackInfoAlignment(value)
      committedAlignment.value = value
    } catch (error) {
      selectedAlignment.value = committedAlignment.value
      notifySettingSaveFailed(t('common.alignment'), error)
    } finally {
      alignmentSaving.value = false
    }
  }

  /** 合并并持久化一次滚动配置变更。 */
  async function updateScrolling(
    patch: Partial<TaskbarTrackInfoScrolling>,
    restorePreview = false,
  ) {
    if (scrollingSaving.value) return
    const nextScrolling = { ...selectedScrolling.value, ...patch }
    selectedScrolling.value = nextScrolling
    scrollingSaving.value = true
    try {
      await setTaskbarTrackInfoScrolling(nextScrolling)
      committedScrolling.value = { ...nextScrolling }
    } catch (error) {
      selectedScrolling.value = { ...committedScrolling.value }
      notifySettingSaveFailed(t('settings.taskbar.trackInfo.scroll'), error)
      if (restorePreview) {
        try {
          await applyTaskbarTrackInfoScrolling(committedScrolling.value)
        } catch (rollbackError) {
          reportBackgroundFailure('恢复之前的歌名滚动配置失败', rollbackError)
        }
      }
    } finally {
      scrollingSaving.value = false
    }
  }

  /** 接收选项卡的外部值并更新滚动方式。 */
  function selectScrollMode(value: unknown) {
    if (isTaskbarTrackInfoScrollMode(value)) void updateScrolling({ mode: value })
  }

  /** 读取 Slider 的单个有效速度值。 */
  function getScrollSpeed(values: number[] | undefined): number | undefined {
    return normalizeTaskbarTrackInfoScrollSpeed(values?.[0])
  }

  /** 限频发送速度预览，避免拖动时产生过密的跨窗口事件。 */
  const previewScrollSpeed = useThrottleFn(
    (speed: number) => {
      applyTaskbarTrackInfoScrolling({ ...selectedScrolling.value, speed }).catch((error) => {
        console.error('预览歌名滚动速度失败', error)
      })
    },
    SCROLL_PREVIEW_INTERVAL_MS,
    true,
    false,
  )

  /** 更新速度状态并实时预览，不写入持久化存储。 */
  function updateScrollSpeed(values: number[] | undefined) {
    if (scrollingSaving.value) return
    const speed = getScrollSpeed(values)
    if (speed === undefined) return
    selectedScrolling.value = { ...selectedScrolling.value, speed }
    void previewScrollSpeed(speed)
  }

  /** 在拖动结束后持久化最终速度。 */
  function commitScrollSpeed(values: number[]) {
    const speed = getScrollSpeed(values)
    if (speed !== undefined) void updateScrolling({ speed }, true)
  }

  onMounted(() => void loadSettings())

  return {
    selectedAlignment: readonly(selectedAlignment),
    alignmentSaving: readonly(alignmentSaving),
    selectedScrolling: readonly(selectedScrolling),
    scrollingSaving: readonly(scrollingSaving),
    selectedVisible: readonly(selectedVisible),
    visibilitySaving: readonly(visibilitySaving),
    updateVisible,
    selectAlignment,
    updateScrolling,
    selectScrollMode,
    updateScrollSpeed,
    commitScrollSpeed,
  }
}
