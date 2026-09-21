import { useTimeoutFn } from '@vueuse/core'
import { toast } from 'vue-sonner'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import { normalizeIpcError, type IpcError } from '@/features/ipc/errors'

import {
  clearCurrentLyricsCache,
  clearLogHistory,
  clearLyricsCache,
  getDataOverview,
  openDataDirectory,
  refreshCurrentLyrics,
  resetConfiguration,
} from './client'
import type { DataDirectoryKind, DataOverview } from './types'

/** 「已清理」提示的保留时长。 */
const STATUS_RESET_DELAY_MS = 2_000

/** 管理数据页的读取和用户操作状态。 */
export function useDataManagement() {
  const { t } = useI18n({ useScope: 'global' })
  const overview = shallowRef<DataOverview | null>(null)
  const loading = shallowRef(false)
  const clearing = shallowRef(false)
  const clearingCurrent = shallowRef(false)
  const refreshingCurrent = shallowRef(false)
  const resetting = shallowRef(false)
  const clearingLogs = shallowRef(false)
  const openingDirectory = shallowRef<DataDirectoryKind | null>(null)
  const cacheCleared = shallowRef(false)
  const logsCleared = shallowRef(false)
  const error = shallowRef<IpcError | null>(null)
  const errorMessage = computed(() => error.value?.message ?? null)
  const { start: scheduleCacheClearedReset, stop: cancelCacheClearedReset } = useTimeoutFn(
    () => (cacheCleared.value = false),
    STATUS_RESET_DELAY_MS,
    { immediate: false },
  )
  const { start: scheduleLogsClearedReset, stop: cancelLogsClearedReset } = useTimeoutFn(
    () => (logsCleared.value = false),
    STATUS_RESET_DELAY_MS,
    { immediate: false },
  )
  let refreshRequestId = 0
  let disposed = false

  /** 刷新三个数据板块的磁盘状态。 */
  async function refresh() {
    const requestId = ++refreshRequestId
    loading.value = true
    try {
      const next = await getDataOverview()
      if (!disposed && requestId === refreshRequestId) {
        overview.value = next
        error.value = null
      }
    } catch (error) {
      if (!disposed && requestId === refreshRequestId) {
        reportBackgroundFailure('读取数据统计失败', error)
        setError(error)
      }
    } finally {
      if (!disposed && requestId === refreshRequestId) loading.value = false
    }
  }

  /** 打开由后端解析的目录，前端不接触可伪造路径。 */
  async function openDirectory(kind: DataDirectoryKind) {
    openingDirectory.value = kind
    try {
      await openDataDirectory(kind)
      error.value = null
    } catch (error) {
      reportBackgroundFailure('打开数据目录失败', error)
      setError(error)
    } finally {
      openingDirectory.value = null
    }
  }

  /** 清理缓存并用后端返回值立即更新容量。 */
  async function clearCache() {
    clearing.value = true
    cacheCleared.value = false
    cancelCacheClearedReset()
    try {
      overview.value = await clearLyricsCache()
      error.value = null
      cacheCleared.value = true
      scheduleCacheClearedReset()
    } catch (error) {
      reportBackgroundFailure('清空歌词缓存失败', error)
      setError(error)
    } finally {
      clearing.value = false
    }
  }

  /** 只删除当前歌曲缓存，不打断当前歌词展示。 */
  async function clearCurrentCache() {
    clearingCurrent.value = true
    try {
      overview.value = await clearCurrentLyricsCache()
      error.value = null
      toast.success(t('settings.data.cache.currentCleared'), {
        description: t('settings.data.cache.currentClearedDescription'),
      })
    } catch (error) {
      reportBackgroundFailure('清除当前歌曲缓存失败', error)
      setError(error)
    } finally {
      clearingCurrent.value = false
    }
  }

  /** 删除当前缓存并让后端立即重新解析当前歌曲。 */
  async function refreshCurrentLyricsData() {
    refreshingCurrent.value = true
    try {
      overview.value = await refreshCurrentLyrics()
      error.value = null
      toast.success(t('settings.data.cache.refreshStarted'), {
        description: t('settings.data.cache.refreshStartedDescription'),
      })
    } catch (error) {
      reportBackgroundFailure('重新获取当前歌词失败', error)
      setError(error)
    } finally {
      refreshingCurrent.value = false
    }
  }

  /** 恢复默认配置；成功后由后端按正常生命周期重启应用。 */
  async function resetConfig() {
    resetting.value = true
    try {
      await resetConfiguration()
      error.value = null
    } catch (error) {
      reportBackgroundFailure('恢复默认配置失败', error)
      setError(error)
      resetting.value = false
    }
  }

  /** 清空历史日志并立即刷新容量，活动日志无需重启即可继续写入。 */
  async function clearLogHistoryFiles() {
    clearingLogs.value = true
    logsCleared.value = false
    cancelLogsClearedReset()
    try {
      overview.value = await clearLogHistory()
      error.value = null
      logsCleared.value = true
      scheduleLogsClearedReset()
    } catch (error) {
      reportBackgroundFailure('清空历史日志失败', error)
      setError(error)
    } finally {
      clearingLogs.value = false
    }
  }

  onActivated(refresh)
  onUnmounted(() => {
    disposed = true
    refreshRequestId += 1
  })

  /** 保留结构化错误，界面当前只展示消息，后续操作可读取是否允许安全重试。 */
  function setError(value: unknown) {
    error.value = normalizeIpcError(value, 'data.unknown')
  }

  return {
    overview: readonly(overview),
    loading: readonly(loading),
    clearing: readonly(clearing),
    clearingCurrent: readonly(clearingCurrent),
    refreshingCurrent: readonly(refreshingCurrent),
    resetting: readonly(resetting),
    clearingLogs: readonly(clearingLogs),
    openingDirectory: readonly(openingDirectory),
    cacheCleared: readonly(cacheCleared),
    logsCleared: readonly(logsCleared),
    error: readonly(error),
    errorMessage,
    refresh,
    openDirectory,
    clearCache,
    clearCurrentCache,
    refreshCurrentLyricsData,
    resetConfig,
    clearLogHistoryFiles,
  }
}
