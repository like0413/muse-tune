import { getVersion } from '@tauri-apps/api/app'
import { check } from '@tauri-apps/plugin-updater'
import { useIntervalFn } from '@vueuse/core'
import { onMounted, onUnmounted } from 'vue'

import { notifyUpdateAvailable } from './notifications'
import {
  getAutomaticUpdateCheck,
  getUpdateCheckFrequency,
  getUpdateCheckInterval,
  getUpdateCheckResult,
  listenUpdateCheckPreferencesChange,
  setUpdateCheckResult,
} from './settings'

const SCHEDULE_REFRESH_INTERVAL = 60 * 60 * 1_000
const UPDATE_CHECK_LOCK = 'muse-tune-automatic-update-check'

/** 在任务栏窗口中按持久化频率执行自动更新检测。 */
export function useAutomaticUpdateMonitor() {
  let checking = false
  let disposed = false
  let unlistenPreferences: (() => void) | undefined

  /** 仅在已启用且距离上次成功检测达到配置周期时访问更新端点。 */
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
        const elapsed = Date.now() - lastResult.checkedAt
        if (
          !enabled ||
          (lastResult.checkedAt > 0 && elapsed >= 0 && elapsed < getUpdateCheckInterval(frequency))
        ) {
          return
        }

        const update = await check({ timeout: 15_000 })
        try {
          const availableVersion = update?.version ?? null
          const shouldNotify =
            availableVersion !== null && availableVersion !== lastResult.availableVersion
          await setUpdateCheckResult({
            checkedAt: Date.now(),
            availableVersion,
          })
          if (shouldNotify) await notifyUpdateAvailable(availableVersion)
        } finally {
          await update?.close()
        }
      })
    } catch (error) {
      console.info('自动检测更新失败，将在下次调度时重试', error)
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
      await checkIfDue()
    } catch (error) {
      console.info('初始化自动更新检测失败', error)
    }
  })

  onUnmounted(() => {
    disposed = true
    pause()
    unlistenPreferences?.()
  })
}
