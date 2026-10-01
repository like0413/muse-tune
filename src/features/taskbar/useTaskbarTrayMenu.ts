import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import {
  getTaskbarAudioSpectrumSettings,
  setTaskbarAudioSpectrumSettings,
} from '@/features/settings/audio-spectrum'
import { getTaskbarLyricsSettings, setTaskbarLyricsSettings } from '@/features/settings/lyrics'

import { TRAY_MENU_ACTION_EVENT, setTrayMenuState } from './client'
import { isTrayMenuAction, type TrayMenuAction, type TrayMenuChecked } from './contracts'

/**
 * 承接托盘菜单：开关动作复用现有设置写入能力，展示状态与文案反向同步回原生菜单。
 * 歌词与频谱勾选来自任务栏设置，自启动勾选由原生插件独立管理。
 */
export function useTaskbarTrayMenu(readSwitchState: () => TrayMenuChecked) {
  const { t } = useI18n({ useScope: 'global' })
  let disposed = false
  let unlisten: UnlistenFn | undefined

  /** 执行菜单操作并集中记录失败，避免菜单回调产生未处理的 Promise。 */
  function runAction(action: () => Promise<void>, failureMessage: string) {
    void action().catch((error) => reportBackgroundFailure(failureMessage, error))
  }

  /** 切换歌词能力，沿用设置模块对原生歌词服务的同步与失败回滚。 */
  async function toggleLyrics() {
    const current = await getTaskbarLyricsSettings()
    await setTaskbarLyricsSettings({ ...current, enabled: !current.enabled })
  }

  /** 切换频谱显示状态，关闭时由现有订阅链自动停止原生音频采集。 */
  async function toggleSpectrum() {
    const current = await getTaskbarAudioSpectrumSettings()
    await setTaskbarAudioSpectrumSettings({ ...current, visible: !current.visible })
  }

  /** 按托盘菜单动作复用与原右键菜单一致的设置写入路径。 */
  function handleAction(action: TrayMenuAction) {
    switch (action) {
      case 'lyrics':
        runAction(toggleLyrics, '切换歌词失败')
        break
      case 'spectrum':
        runAction(toggleSpectrum, '切换频谱显示失败')
        break
    }
  }

  /** 把当前开关状态与界面语言推送给托盘原生菜单。 */
  function pushMenuState(checked: TrayMenuChecked) {
    // 文案在 effect 内同步求值：切换界面语言时会重新推送菜单标题。
    const labels = {
      autostart: t('settings.general.autostart.title'),
      lyrics: t('taskbar.menu.lyrics'),
      spectrum: t('taskbar.menu.spectrum'),
      settings: t('taskbar.menu.settings'),
      restart: import.meta.env.PROD
        ? t('taskbar.menu.restart')
        : t('taskbar.menu.restartProductionOnly'),
      quit: t('taskbar.menu.quit'),
    }
    return setTrayMenuState({ labels, checked })
  }

  onMounted(async () => {
    try {
      const stopListener = await listen<string>(TRAY_MENU_ACTION_EVENT, ({ payload }) => {
        if (isTrayMenuAction(payload)) handleAction(payload)
      })
      if (disposed) {
        stopListener()
        return
      }
      unlisten = stopListener
    } catch (error) {
      reportBackgroundFailure('监听托盘菜单动作失败', error)
    }
  })

  watchEffect(() => {
    runAction(() => pushMenuState(readSwitchState()), '同步托盘菜单状态失败')
  })

  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })
}
