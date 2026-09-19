import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import {
  getTaskbarAudioSpectrumSettings,
  setTaskbarAudioSpectrumSettings,
} from '@/features/settings/audio-spectrum'
import {
  getTaskbarCoverAppearance,
  isTaskbarCoverVisibleInMode,
  setTaskbarCoverAppearance,
  updateTaskbarCoverModeVisibility,
  type TaskbarCoverDisplayMode,
} from '@/features/settings/cover'
import { getTaskbarLyricsSettings, setTaskbarLyricsSettings } from '@/features/settings/lyrics'

import { TRAY_MENU_ACTION_EVENT, setTrayMenuState } from './client'
import { isTrayMenuAction, type TrayMenuAction, type TrayMenuChecked } from './contracts'

/**
 * 承接托盘菜单：开关动作复用现有设置写入能力，展示状态与文案反向同步回原生菜单。
 * 原生菜单只渲染，勾选与文案的唯一来源仍是任务栏窗口持有的设置状态。
 */
export function useTaskbarTrayMenu(readSwitchState: () => TrayMenuChecked) {
  const { t } = useI18n({ useScope: 'global' })
  let disposed = false
  let unlisten: UnlistenFn | undefined

  /** 执行菜单操作并集中记录失败，避免菜单回调产生未处理的 Promise。 */
  function runAction(action: () => Promise<void>, failureMessage: string) {
    void action().catch((error) => console.error(failureMessage, error))
  }

  /** 切换指定界面模式的封面显示，并保留另一个模式当前状态。 */
  async function toggleCover(mode: TaskbarCoverDisplayMode) {
    const current = await getTaskbarCoverAppearance()
    const visible = isTaskbarCoverVisibleInMode(current.visibility, mode)
    await setTaskbarCoverAppearance({
      ...current,
      visibility: updateTaskbarCoverModeVisibility(current.visibility, mode, !visible),
    })
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
      case 'normal-cover':
        runAction(() => toggleCover('normal'), '切换普通模式封面失败')
        break
      case 'lyrics-cover':
        runAction(() => toggleCover('lyrics'), '切换歌词模式封面失败')
        break
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
      normalCover: t('taskbar.menu.normalCover'),
      lyricsCover: t('taskbar.menu.lyricsCover'),
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
      console.error('监听托盘菜单动作失败', error)
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
