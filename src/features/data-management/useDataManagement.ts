import { toast } from 'vue-sonner'

import { normalizeIpcError, type IpcError } from '@/features/ipc/errors'

import {
  clearLogHistory,
  clearCurrentLyricsCache,
  clearLyricsCache,
  getDataOverview,
  openDataDirectory,
  refreshCurrentLyrics,
  resetConfiguration,
} from './client'
import type { DataDirectoryKind, DataOverview } from './types'

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
  const error = shallowRef<IpcError | null>(null)
  const errorMessage = computed(() => error.value?.message ?? null)
  let refreshRequestId = 0
  let clearStatusTimer: number | undefined
  let logClearStatusTimer: number | undefined
  const logsCleared = shallowRef(false)
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
      if (!disposed && requestId === refreshRequestId) setError(error)
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
      setError(error)
    } finally {
      openingDirectory.value = null
    }
  }

  /** 清理缓存并用后端返回值立即更新容量。 */
  async function clearCache() {
    clearing.value = true
    cacheCleared.value = false
    window.clearTimeout(clearStatusTimer)
    try {
      overview.value = await clearLyricsCache()
      error.value = null
      cacheCleared.value = true
      clearStatusTimer = window.setTimeout(() => (cacheCleared.value = false), 2_000)
    } catch (error) {
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
      setError(error)
      resetting.value = false
    }
  }

  /** 清空历史日志并立即刷新容量，活动日志无需重启即可继续写入。 */
  async function clearLogHistoryFiles() {
    clearingLogs.value = true
    logsCleared.value = false
    window.clearTimeout(logClearStatusTimer)
    try {
      overview.value = await clearLogHistory()
      error.value = null
      logsCleared.value = true
      logClearStatusTimer = window.setTimeout(() => (logsCleared.value = false), 2_000)
    } catch (error) {
      setError(error)
    } finally {
      clearingLogs.value = false
    }
  }

  onActivated(refresh)
  onUnmounted(() => {
    disposed = true
    refreshRequestId += 1
    window.clearTimeout(clearStatusTimer)
    window.clearTimeout(logClearStatusTimer)
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
