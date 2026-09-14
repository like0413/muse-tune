import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import { useDebounceFn } from '@vueuse/core'

import { LYRICS_DIAGNOSTICS_CHANGED_EVENT } from '@/features/lyrics/client'
import { MEDIA_SESSION_CHANGED_EVENT } from '@/features/media/client'

import { getDiagnostics } from './client'
import { createSanitizedDiagnosticsReport } from './report'
import type { DiagnosticsSnapshot } from './types'

const DIAGNOSTICS_REFRESH_EVENTS = [
  MEDIA_SESSION_CHANGED_EVENT,
  LYRICS_DIAGNOSTICS_CHANGED_EVENT,
] as const

/** 使用现有业务事件刷新应用诊断，不引入定时轮询。 */
export function useDiagnostics() {
  const diagnostics = shallowRef<DiagnosticsSnapshot | null>(null)
  const refreshing = shallowRef(false)
  const errorMessage = shallowRef<string | null>(null)
  const updatedAt = shallowRef<Date | null>(null)
  const reportCopied = shallowRef(false)
  let disposed = false
  let lifecycleId = 0
  let requestRunning = false
  let refreshQueued = false
  let storageRefreshQueued = false
  let unlisteners: UnlistenFn[] = []

  /** 合并歌曲切换时相邻的媒体与歌词事件，避免重复读取磁盘和媒体线程。 */
  const scheduleRefresh = useDebounceFn(
    () => {
      if (!disposed) void refresh(false)
    },
    150,
    { maxWait: 500 },
  )

  /** 串行合并刷新；执行期间的新事件最多追加一次后续采集。 */
  async function refresh(includeStorage: boolean) {
    refreshQueued = true
    storageRefreshQueued ||= includeStorage
    if (!disposed) refreshing.value = true
    if (requestRunning) return

    requestRunning = true
    try {
      while (refreshQueued) {
        refreshQueued = false
        const refreshStorage = storageRefreshQueued
        storageRefreshQueued = false
        const currentLifecycle = lifecycleId
        try {
          const next = await getDiagnostics(refreshStorage)
          if (!disposed && currentLifecycle === lifecycleId) {
            diagnostics.value = next
            errorMessage.value = null
            updatedAt.value = new Date()
          }
        } catch (error) {
          if (!disposed && currentLifecycle === lifecycleId) {
            errorMessage.value = error instanceof Error ? error.message : String(error)
          }
        }
      }
    } finally {
      requestRunning = false
      if (!disposed) refreshing.value = false
    }
  }

  /** 用户主动刷新时同时更新低频存储统计。 */
  function refreshAll() {
    return refresh(true)
  }

  /** 先订阅再读取，覆盖诊断页初始化期间的状态变化。 */
  async function initialize() {
    const currentLifecycle = ++lifecycleId
    try {
      const listeners = await Promise.all(
        DIAGNOSTICS_REFRESH_EVENTS.map((event) => listen(event, scheduleRefresh)),
      )
      if (disposed || currentLifecycle !== lifecycleId) {
        listeners.forEach((unlisten) => unlisten())
        return
      }
      unlisteners = listeners
      await refresh(true)
    } catch (error) {
      if (!disposed && currentLifecycle === lifecycleId) {
        errorMessage.value = error instanceof Error ? error.message : String(error)
      }
    }
  }

  /** 页面被 KeepAlive 隐藏时释放事件，避免后台持续采集诊断。 */
  function dispose() {
    disposed = true
    lifecycleId += 1
    refreshQueued = false
    storageRefreshQueued = false
    unlisteners.forEach((unlisten) => unlisten())
    unlisteners = []
    refreshing.value = false
  }

  /** 始终复制脱敏报告，避免无意带出歌曲、账号路径和进程标识。 */
  async function copyReport() {
    if (!diagnostics.value) return
    try {
      await navigator.clipboard.writeText(createSanitizedDiagnosticsReport(diagnostics.value))
      reportCopied.value = true
      window.setTimeout(() => (reportCopied.value = false), 2_000)
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : String(error)
    }
  }

  onActivated(() => {
    disposed = false
    void initialize()
  })
  onDeactivated(dispose)
  onUnmounted(dispose)

  return {
    diagnostics: readonly(diagnostics),
    refreshing: readonly(refreshing),
    errorMessage: readonly(errorMessage),
    updatedAt: readonly(updatedAt),
    reportCopied: readonly(reportCopied),
    refresh: refreshAll,
    copyReport,
  }
}
