import type { UnlistenFn } from '@tauri-apps/api/event'
import { confirm } from '@tauri-apps/plugin-dialog'
import { openUrl } from '@tauri-apps/plugin-opener'
import type { DownloadEvent, Update } from '@tauri-apps/plugin-updater'
import { check } from '@tauri-apps/plugin-updater'

import { getErrorMessage, reportBackgroundFailure } from '@/features/feedback/errors'
import { logDebug } from '@/features/logging'
import { PROJECT_RELEASES_URL } from '@/features/project/metadata'
import { getRuntimeEnvironment } from '@/features/runtime/environment'

import { createDownloadProgressTracker } from './download-progress'
import {
  getUpdateCheckResult,
  listenUpdateCheckResultChange,
  setUpdateCheckResult,
  type UpdateCheckResult,
} from './settings'
import type { AvailableUpdateView, UpdateStatus } from './types'

/** 管理关于页的版本读取、更新检查和安装生命周期。 */
export function useApplicationUpdater() {
  const { t } = useI18n({ useScope: 'global' })
  const status = shallowRef<UpdateStatus>('idle')
  const isChecking = shallowRef(false)
  const isDownloading = shallowRef(false)
  const availableUpdate = shallowRef<Update | null>(null)
  const detectedVersion = shallowRef<string | null>(null)
  const downloadProgress = shallowRef<number | null>(null)
  const errorMessage = shallowRef<string | null>(null)
  const isPortable = shallowRef(false)
  let initialized = false
  let disposed = false
  let resultRevision = 0
  let unlistenResult: UnlistenFn | undefined
  let activeCheck: Promise<Update | null> | null = null
  let installRequestActive = false
  const trackDownloadProgress = createDownloadProgressTracker()

  const statusLabel = computed(() => {
    switch (status.value) {
      case 'latest':
        return t('settings.about.update.latest')
      case 'detected':
      case 'available':
        return t('settings.about.update.available')
      case 'error':
        return t('settings.about.update.checkFailed')
      default:
        return null
    }
  })
  const availableUpdateView = computed<AvailableUpdateView | null>(() => {
    const update = availableUpdate.value
    if (!update) return null
    return {
      currentVersion: update.currentVersion,
      version: update.version,
      date: update.date ?? null,
    }
  })

  /** 释放上一次检查返回的原生资源。 */
  function replaceAvailableUpdate(next: Update | null) {
    const previous = availableUpdate.value
    availableUpdate.value = next
    if (previous && previous !== next) {
      void previous.close().catch((error) => console.debug('释放更新资源失败', error))
    }
  }

  /** 将后台检测结果映射为关于页状态，但不覆盖正在进行的手动操作。 */
  function applyAutomaticResult(result: UpdateCheckResult) {
    if (availableUpdate.value || isChecking.value || isDownloading.value) return
    detectedVersion.value = result.availableVersion
    status.value = result.checkedAt === 0 ? 'idle' : result.availableVersion ? 'detected' : 'latest'
  }

  /** 合并同一时刻的更新请求，避免页面静默检查与手动检查重复访问网络。 */
  function requestUpdateCheck() {
    if (activeCheck) return activeCheck
    // 单次检查的请求超时；超时按检查失败处理，静默路径只留调试通道。
    activeCheck = check({ timeout: 15_000 }).finally(() => {
      activeCheck = null
    })
    return activeCheck
  }

  /** 通过官方 Updater 检查 GitHub 发布的新版本。 */
  async function checkForUpdates(options: { silent?: boolean } = {}) {
    if (isChecking.value || isDownloading.value) return
    const silent = options.silent === true
    if (!silent) {
      isChecking.value = true
      errorMessage.value = null
      downloadProgress.value = null
    }
    try {
      const update = await requestUpdateCheck()
      if (disposed) {
        await update?.close()
        return
      }
      replaceAvailableUpdate(update)
      detectedVersion.value = update?.version ?? null
      status.value = update ? 'available' : 'latest'
      errorMessage.value = null
      const checkedAt = Date.now()
      void setUpdateCheckResult({
        checkedAt,
        attemptedAt: checkedAt,
        consecutiveFailures: 0,
        availableVersion: update?.version ?? null,
      }).catch((error) => console.debug('保存更新检测结果失败', error))
    } catch (error) {
      if (silent) {
        // 静默路径不打扰用户，失败也属预期（离线）：只留调试通道，不进日志文件。
        logDebug('静默检查更新失败', error)
        return
      }
      status.value = 'error'
      reportBackgroundFailure('检查更新失败', error)
      errorMessage.value = getErrorMessage(error, t('settings.about.update.checkFailed'))
    } finally {
      if (!silent) isChecking.value = false
    }
  }

  /** 使用系统默认浏览器打开完整的 GitHub Releases 历史。 */
  async function openReleaseNotes() {
    try {
      await openUrl(PROJECT_RELEASES_URL)
    } catch (error) {
      reportBackgroundFailure('打开更新日志失败', error)
      errorMessage.value = getErrorMessage(error, t('settings.about.update.openReleaseNotesFailed'))
    }
  }

  /** 汇总下载事件为稳定的百分比状态；内容长度未知时显示不确定进度。 */
  function handleDownloadEvent(event: DownloadEvent) {
    downloadProgress.value = trackDownloadProgress(event)
  }

  /** 下载并安装；Windows 安装器启动成功后会接管退出和重启。 */
  async function performInstallUpdate(requireConfirmation: boolean) {
    const update = availableUpdate.value
    if (!update || installRequestActive || isDownloading.value) return
    installRequestActive = true
    try {
      const environment = await getRuntimeEnvironment()
      isPortable.value = environment.portable
      if (environment.portable) {
        await openReleaseNotes()
        return
      }
      if (requireConfirmation) {
        const shouldInstall = await confirm(t('settings.about.update.installConfirmDescription'), {
          title: t('settings.about.update.installConfirmTitle'),
          kind: 'warning',
          okLabel: t('common.continue'),
          cancelLabel: t('common.cancel'),
        })
        if (!shouldInstall) return
      }
      status.value = 'available'
      isDownloading.value = true
      errorMessage.value = null
      downloadProgress.value = null
      await update.downloadAndInstall(handleDownloadEvent, { restartAfterInstall: true })
    } catch (error) {
      status.value = 'available'
      reportBackgroundFailure('安装更新失败', error)
      const reason = getErrorMessage(error, t('settings.about.update.installerFailed'))
      errorMessage.value = t('settings.about.update.installFailed', { reason })
    } finally {
      isDownloading.value = false
      installRequestActive = false
    }
  }

  /** 设置页按钮保留原有二次确认。 */
  function installUpdate() {
    return performInstallUpdate(true)
  }

  /** 托盘更新项本身就是用户确认，检查到可用版本后直接下载并安装。 */
  async function installUpdateAutomatically() {
    await initialize()
    if (disposed) return
    if (!availableUpdate.value) await checkForUpdates()
    if (!disposed && availableUpdate.value) await performInstallUpdate(false)
  }

  /** 首次进入关于页时读取最近结果并订阅后台检测结果。 */
  async function initialize() {
    if (initialized) return
    initialized = true
    try {
      const stopListener = await listenUpdateCheckResultChange((result) => {
        resultRevision += 1
        applyAutomaticResult(result)
      })
      if (disposed) {
        stopListener()
        return
      }
      unlistenResult = stopListener
      const revisionBeforeRead = resultRevision
      const [result, environment] = await Promise.all([
        getUpdateCheckResult(),
        getRuntimeEnvironment(),
      ])
      if (disposed) return
      isPortable.value = environment.portable
      if (resultRevision === revisionBeforeRead) applyAutomaticResult(result)
    } catch (error) {
      status.value = 'error'
      reportBackgroundFailure('初始化更新设置失败', error)
      errorMessage.value = getErrorMessage(error, t('settings.about.update.initializeFailed'))
    }
  }

  /** 进入关于页只同步已保存的结果，后台调度负责定期检查。 */
  onActivated(() => void initialize())
  onUnmounted(() => {
    disposed = true
    unlistenResult?.()
    replaceAvailableUpdate(null)
  })

  return {
    status: readonly(status),
    statusLabel,
    isChecking: readonly(isChecking),
    isDownloading: readonly(isDownloading),
    availableUpdate: availableUpdateView,
    detectedVersion: readonly(detectedVersion),
    downloadProgress: readonly(downloadProgress),
    errorMessage: readonly(errorMessage),
    isPortable: readonly(isPortable),
    checkForUpdates,
    openReleaseNotes,
    installUpdate,
    installUpdateAutomatically,
  }
}
