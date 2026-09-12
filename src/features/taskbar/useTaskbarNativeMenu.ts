import { invoke } from '@tauri-apps/api/core'
import {
  CheckMenuItem,
  Menu,
  MenuItem,
  PredefinedMenuItem,
  type CheckMenuItemOptions,
} from '@tauri-apps/api/menu'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { onUnmounted } from 'vue'

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

interface NativeMenuResources {
  menu: Menu
  normalCover: CheckMenuItem
  lyricsCover: CheckMenuItem
  lyrics: CheckMenuItem
  spectrum: CheckMenuItem
  separator: PredefinedMenuItem
  settings: MenuItem
  restart: MenuItem
  quit: PredefinedMenuItem
}

/** 管理任务栏原生右键菜单，并直接复用现有设置读写与广播能力。 */
export function useTaskbarNativeMenu() {
  let resources: NativeMenuResources | undefined
  let resourcesPromise: Promise<NativeMenuResources> | undefined
  let showing = false
  let disposed = false

  /** 执行菜单操作并集中记录失败，避免原生菜单回调产生未处理的 Promise。 */
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

  /** 创建一个窗口内复用的原生菜单，避免每次右键重复分配系统资源。 */
  async function createResources(): Promise<NativeMenuResources> {
    const createCheckItem = (options: CheckMenuItemOptions) => CheckMenuItem.new(options)
    const [normalCover, lyricsCover, lyrics, spectrum, separator, settings, restart, quit] =
      await Promise.all([
        createCheckItem({
          id: 'taskbar-toggle-normal-cover',
          text: '普通模式封面',
          action: () => runAction(() => toggleCover('normal'), '切换普通模式封面失败'),
        }),
        createCheckItem({
          id: 'taskbar-toggle-lyrics-cover',
          text: '歌词模式封面',
          action: () => runAction(() => toggleCover('lyrics'), '切换歌词模式封面失败'),
        }),
        createCheckItem({
          id: 'taskbar-toggle-lyrics',
          text: '开启歌词',
          action: () => runAction(toggleLyrics, '切换歌词失败'),
        }),
        createCheckItem({
          id: 'taskbar-toggle-spectrum',
          text: '显示频谱',
          action: () => runAction(toggleSpectrum, '切换频谱显示失败'),
        }),
        PredefinedMenuItem.new({ item: 'Separator' }),
        MenuItem.new({
          id: 'taskbar-open-settings',
          text: '打开设置',
          action: () =>
            runAction(() => invoke<void>('open_settings_window'), '从任务栏菜单打开设置失败'),
        }),
        MenuItem.new({
          id: 'taskbar-restart-application',
          text: import.meta.env.PROD ? '重启应用' : '重启应用（正式版可用）',
          enabled: import.meta.env.PROD,
          action: () =>
            runAction(() => invoke<void>('restart_application'), '从任务栏菜单重启应用失败'),
        }),
        PredefinedMenuItem.new({ item: 'Quit', text: '退出应用' }),
      ])
    const menu = await Menu.new({
      items: [normalCover, lyricsCover, lyrics, spectrum, separator, settings, restart, quit],
    })
    return { menu, normalCover, lyricsCover, lyrics, spectrum, separator, settings, restart, quit }
  }

  /** 获取当前窗口唯一的菜单实例，并合并并发初始化。 */
  async function getResources(): Promise<NativeMenuResources> {
    if (resources) return resources
    resourcesPromise ??= createResources()
    const created = await resourcesPromise
    if (disposed) {
      await closeResources(created)
      throw new Error('任务栏窗口已关闭')
    }
    resources = created
    return created
  }

  /** 在弹出前同步四个勾选状态，确保多窗口修改后菜单仍显示最新值。 */
  async function refreshCheckedState(current: NativeMenuResources) {
    const [cover, lyrics, spectrum] = await Promise.all([
      getTaskbarCoverAppearance(),
      getTaskbarLyricsSettings(),
      getTaskbarAudioSpectrumSettings(),
    ])
    await Promise.all([
      current.normalCover.setChecked(isTaskbarCoverVisibleInMode(cover.visibility, 'normal')),
      current.lyricsCover.setChecked(isTaskbarCoverVisibleInMode(cover.visibility, 'lyrics')),
      current.lyrics.setChecked(lyrics.enabled),
      current.spectrum.setChecked(spectrum.visible),
    ])
  }

  /** 在鼠标当前位置显示 Windows 原生菜单；重复右键不会并行打开多个菜单。 */
  async function show() {
    if (showing || disposed) return
    showing = true
    try {
      const current = await getResources()
      await refreshCheckedState(current)
      await current.menu.popup(undefined, getCurrentWindow())
    } catch (error) {
      console.error('显示任务栏原生菜单失败', error)
    } finally {
      showing = false
    }
  }

  /** 释放菜单及其子项持有的原生资源。 */
  async function closeResources(current: NativeMenuResources) {
    await Promise.allSettled([
      current.menu.close(),
      current.normalCover.close(),
      current.lyricsCover.close(),
      current.lyrics.close(),
      current.spectrum.close(),
      current.separator.close(),
      current.settings.close(),
      current.restart.close(),
      current.quit.close(),
    ])
  }

  onUnmounted(() => {
    disposed = true
    const current = resources
    resources = undefined
    if (current) void closeResources(current)
  })

  return { show }
}
