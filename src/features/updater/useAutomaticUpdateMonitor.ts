import { getVersion } from '@tauri-apps/api/app'
import { check } from '@tauri-apps/plugin-updater'
import { useTimeoutFn } from '@vueuse/core'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import { logDebug, logWarn } from '@/features/logging'

import { notifyUpdateAvailable } from './notifications'
import {
  getUpdateCheckResult,
  listenUpdateCheckResultChange,
  setUpdateCheckResult,
  type UpdateCheckResult,
} from './settings'
import { setUpdateTrayState } from './tray'

const STARTUP_DELAY = 30_000
const CHECK_INTERVAL = 24 * 60 * 60 * 1_000
const FIRST_RETRY_INTERVAL = 60 * 60 * 1_000
const REPEATED_RETRY_INTERVAL = 6 * 60 * 60 * 1_000
const UPDATE_CHECK_LOCK = 'muse-tune-automatic-update-check'

/** 成功后每天检查；首次失败隔一小时，后续失败隔六小时重试。 */
function nextCheckAt(result: UpdateCheckResult): number {
  const interval =
    result.consecutiveFailures > 1
      ? REPEATED_RETRY_INTERVAL
      : result.consecutiveFailures === 1
        ? FIRST_RETRY_INTERVAL
        : CHECK_INTERVAL
  const lastAttempt = Math.max(result.checkedAt, result.attemptedAt)
  return lastAttempt > 0 ? Math.min(lastAttempt, Date.now()) + interval : 0
}

/** 在任务栏窗口中按固定策略执行自动更新检测。 */
export function useAutomaticUpdateMonitor() {
  const { t } = useI18n({ useScope: 'global' })
  const detectedVersion = shallowRef<string | null>(null)
  const trayStateReady = shallowRef(false)
  let checking = false
  let disposed = false
  const startupReadyAt = Date.now() + STARTUP_DELAY
  const checkDelay = shallowRef(STARTUP_DELAY)
  let unlistenResult: (() => void) | undefined

  /** 用检测结果驱动唯一的原生托盘入口，界面语言变化时同步刷新文案。 */
  watchEffect(() => {
    if (!trayStateReady.value) return
    const version = detectedVersion.value
    const presentation = version
      ? {
          label: t('taskbar.menu.updateAvailable', { version }),
          tooltip: t('taskbar.menu.updateTooltip', { version }),
        }
      : { label: null, tooltip: null }
    void setUpdateTrayState(presentation).catch((error) => logWarn('同步托盘更新入口失败', error))
  })

  /**
   * 失败也必须记下尝试时间。
   *
   * 只记成功时间时，检测失败的条目永远处于“已到期”状态：每个 bar 窗口挂载、每次调度
   * 都会重新请求一个已经失败的端点，既反复打网络，也把同一条错误日志刷满整个日志预算。
   */
  async function recordFailedAttempt() {
    try {
      const lastResult = await getUpdateCheckResult()
      await setUpdateCheckResult({
        ...lastResult,
        attemptedAt: Date.now(),
        consecutiveFailures: lastResult.consecutiveFailures + 1,
      })
    } catch (error) {
      logWarn('记录更新检测尝试时间失败', error)
    }
  }

  /** 在启动延迟和持久化检查期限都到期后，访问更新端点。 */
  async function checkIfDue() {
    if (checking || disposed) return
    checking = true
    let failed = false
    try {
      // 多显示器会创建多个 bar，通过浏览器原生锁保证同一时刻仅一个窗口访问更新端点。
      await navigator.locks.request(UPDATE_CHECK_LOCK, async () => {
        if (disposed) return
        const lastResult = await getUpdateCheckResult()
        const currentVersion = await getVersion()
        if (lastResult.availableVersion === currentVersion) {
          await setUpdateCheckResult({ ...lastResult, availableVersion: null })
          detectedVersion.value = null
          trayStateReady.value = true
          return
        }
        detectedVersion.value = lastResult.availableVersion
        trayStateReady.value = true
        if (Date.now() < Math.max(startupReadyAt, nextCheckAt(lastResult))) return

        // 单次检查的请求超时；失败会记下尝试时间，因此要等到下个周期才会重试。
        const update = await check({ timeout: 15_000 }).catch(async (error) => {
          // 在跨窗口锁内记录失败，其他窗口取得锁后才能读到新的重试期限。
          await recordFailedAttempt()
          throw error
        })
        try {
          const availableVersion = update?.version ?? null
          const shouldNotify =
            availableVersion !== null && availableVersion !== lastResult.availableVersion
          const checkedAt = Date.now()
          await setUpdateCheckResult({
            checkedAt,
            attemptedAt: checkedAt,
            availableVersion,
            consecutiveFailures: 0,
          })
          detectedVersion.value = availableVersion
          if (shouldNotify) {
            try {
              await notifyUpdateAvailable(availableVersion)
            } catch (error) {
              logWarn('发送新版本通知失败', error)
            }
          }
        } finally {
          await update?.close()
        }
      })
    } catch (error) {
      failed = true
      // 离线时这是预期结果：只留调试通道，不进日志文件，也不与真实故障争夺注意力。
      logDebug('自动检测更新失败，将在下个周期重试', error)
    } finally {
      checking = false
      if (!disposed) {
        try {
          scheduleNextCheck(await getUpdateCheckResult())
          // 存储或原生调用异常时也避免立即反复重试。
          if (failed && checkDelay.value < FIRST_RETRY_INTERVAL) {
            checkDelay.value = FIRST_RETRY_INTERVAL
            start()
          }
        } catch (error) {
          logWarn('读取更新调度时间失败', error)
          checkDelay.value = FIRST_RETRY_INTERVAL
          start()
        }
      }
    }
  }

  const { start, stop } = useTimeoutFn(() => void checkIfDue(), checkDelay, { immediate: false })

  /** 只安排下一次到期检查，结果变化时重新计算，不按小时轮询。 */
  function scheduleNextCheck(result: UpdateCheckResult) {
    if (disposed || checking) return
    checkDelay.value = Math.max(1, Math.max(startupReadyAt, nextCheckAt(result)) - Date.now())
    start()
  }

  /** 启动时先恢复托盘入口并监听跨窗口结果，网络请求至少延迟三十秒。 */
  onMounted(async () => {
    try {
      const stopResult = await listenUpdateCheckResultChange((result) => {
        detectedVersion.value = result.availableVersion
        trayStateReady.value = true
        scheduleNextCheck(result)
      })
      if (disposed) {
        stopResult()
        return
      }
      unlistenResult = stopResult
      const result = await getUpdateCheckResult()
      if (disposed) return
      const currentVersion = await getVersion()
      detectedVersion.value =
        result.availableVersion === currentVersion ? null : result.availableVersion
      trayStateReady.value = true
      scheduleNextCheck(result)
    } catch (error) {
      reportBackgroundFailure('初始化自动更新调度失败', error)
      if (!disposed) {
        checkDelay.value = FIRST_RETRY_INTERVAL
        start()
      }
    }
  })

  onUnmounted(() => {
    disposed = true
    stop()
    unlistenResult?.()
  })
}
