import { getVersion } from '@tauri-apps/api/app'
import { check } from '@tauri-apps/plugin-updater'
import { useIntervalFn } from '@vueuse/core'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import { logDebug, logWarn } from '@/features/logging'

import { notifyUpdateAvailable } from './notifications'
import {
  getAutomaticUpdateCheck,
  getUpdateCheckFrequency,
  getUpdateCheckInterval,
  getUpdateCheckResult,
  listenUpdateCheckPreferencesChange,
  setUpdateCheckResult,
  type UpdateCheckResult,
} from './settings'

const SCHEDULE_REFRESH_INTERVAL = 60 * 60 * 1_000
const UPDATE_CHECK_LOCK = 'muse-tune-automatic-update-check'

/** 上次尝试时间：成功与失败共用同一节流依据，失败后同样要等到下个周期再重试。 */
function lastAttemptAt(result: UpdateCheckResult): number {
  return Math.max(result.checkedAt, result.attemptedAt)
}

/** 在任务栏窗口中按持久化频率执行自动更新检测。 */
export function useAutomaticUpdateMonitor() {
  let checking = false
  let disposed = false
  let unlistenPreferences: (() => void) | undefined

  /**
   * 失败也必须记下尝试时间。
   *
   * 只记成功时间时，检测失败的条目永远处于“已到期”状态：每个 bar 窗口挂载、每次调度
   * 都会重新请求一个已经失败的端点，既反复打网络，也把同一条错误日志刷满整个日志预算。
   */
  async function recordFailedAttempt() {
    try {
      const lastResult = await getUpdateCheckResult()
      await setUpdateCheckResult({ ...lastResult, attemptedAt: Date.now() })
    } catch (error) {
      logWarn('记录更新检测尝试时间失败', error)
    }
  }

  /** 仅在已启用且距离上次检测达到配置周期时访问更新端点。 */
  async function checkIfDue() {
    if (checking || disposed) return
    checking = true
    try {
      // 多显示器会创建多个 bar，通过浏览器原生锁保证同一时刻仅一个窗口访问更新端点。
      await navigator.locks.request(UPDATE_CHECK_LOCK, { ifAvailable: true }, async (lock) => {
        if (!lock || disposed) return
        const [enabled, frequency, lastResult] = await Promise.all([
          getAutomaticUpdateCheck(),
          getUpdateCheckFrequency(),
          getUpdateCheckResult(),
        ])
        const currentVersion = await getVersion()
        if (lastResult.availableVersion === currentVersion) {
          await setUpdateCheckResult({ ...lastResult, availableVersion: null })
          return
        }
        const lastAttempt = lastAttemptAt(lastResult)
        const elapsed = Date.now() - lastAttempt
        if (
          !enabled ||
          (lastAttempt > 0 && elapsed >= 0 && elapsed < getUpdateCheckInterval(frequency))
        ) {
          return
        }

        const update = await check({ timeout: 15_000 })
        try {
          const availableVersion = update?.version ?? null
          const shouldNotify =
            availableVersion !== null && availableVersion !== lastResult.availableVersion
          const checkedAt = Date.now()
          await setUpdateCheckResult({ checkedAt, attemptedAt: checkedAt, availableVersion })
          if (shouldNotify) await notifyUpdateAvailable(availableVersion)
        } finally {
          await update?.close()
        }
      })
    } catch (error) {
      await recordFailedAttempt()
      // 离线时这是预期结果：只留调试通道，不进日志文件，也不与真实故障争夺注意力。
      logDebug('自动检测更新失败，将在下个周期重试', error)
    } finally {
      checking = false
    }
  }

  const { pause } = useIntervalFn(() => void checkIfDue(), SCHEDULE_REFRESH_INTERVAL)

  /** 先监听配置变化，再执行启动检查，避免设置保存期间错过事件。 */
  onMounted(async () => {
    try {
      const stopListener = await listenUpdateCheckPreferencesChange(() => void checkIfDue())
      if (disposed) {
        stopListener()
        return
      }
      unlistenPreferences = stopListener
    } catch (error) {
      // 监听注册失败会让自动检测永久失效，属于真实故障，不能和“离线检测失败”混为一谈。
      reportBackgroundFailure('注册自动更新检测监听失败', error)
      return
    }
    await checkIfDue()
  })

  onUnmounted(() => {
    disposed = true
    pause()
    unlistenPreferences?.()
  })
}
