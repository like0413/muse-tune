import { onActivated, onUnmounted, readonly, shallowRef } from 'vue'

import {
  clearLogHistory,
  clearLyricsCache,
  getDataOverview,
  openDataDirectory,
  resetConfiguration,
} from './api'
import type { DataDirectoryKind, DataOverview } from './types'

/** 管理数据页的读取和用户操作状态。 */
export function useDataManagement() {
  const overview = shallowRef<DataOverview | null>(null)
  const loading = shallowRef(false)
  const clearing = shallowRef(false)
  const resetting = shallowRef(false)
  const clearingLogs = shallowRef(false)
  const openingDirectory = shallowRef<DataDirectoryKind | null>(null)
  const cacheCleared = shallowRef(false)
  const errorMessage = shallowRef<string | null>(null)
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
        errorMessage.value = null
      }
    } catch (error) {
      if (!disposed && requestId === refreshRequestId) errorMessage.value = String(error)
    } finally {
      if (!disposed && requestId === refreshRequestId) loading.value = false
    }
  }

  /** 打开由后端解析的目录，前端不接触可伪造路径。 */
  async function openDirectory(kind: DataDirectoryKind) {
    openingDirectory.value = kind
    try {
      await openDataDirectory(kind)
      errorMessage.value = null
    } catch (error) {
      errorMessage.value = String(error)
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
      errorMessage.value = null
      cacheCleared.value = true
      clearStatusTimer = window.setTimeout(() => (cacheCleared.value = false), 2_000)
    } catch (error) {
      errorMessage.value = String(error)
    } finally {
      clearing.value = false
    }
  }

  /** 恢复默认配置；成功后由后端按正常生命周期重启应用。 */
  async function resetConfig() {
    resetting.value = true
    try {
      await resetConfiguration()
      errorMessage.value = null
    } catch (error) {
      errorMessage.value = String(error)
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
      errorMessage.value = null
      logsCleared.value = true
      logClearStatusTimer = window.setTimeout(() => (logsCleared.value = false), 2_000)
    } catch (error) {
      errorMessage.value = String(error)
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

  return {
    overview: readonly(overview),
    loading: readonly(loading),
    clearing: readonly(clearing),
    resetting: readonly(resetting),
    clearingLogs: readonly(clearingLogs),
    openingDirectory: readonly(openingDirectory),
    cacheCleared: readonly(cacheCleared),
    logsCleared: readonly(logsCleared),
    errorMessage: readonly(errorMessage),
    refresh,
    openDirectory,
    clearCache,
    resetConfig,
    clearLogHistoryFiles,
  }
}
