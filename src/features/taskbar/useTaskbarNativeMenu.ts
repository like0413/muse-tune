import {
  CheckMenuItem,
  Menu,
  MenuItem,
  PredefinedMenuItem,
  type CheckMenuItemOptions,
} from '@tauri-apps/api/menu'
import { getCurrentWindow } from '@tauri-apps/api/window'

import { openSettingsWindow } from '@/features/application/client'
import { i18n } from '@/features/i18n'
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
import { restartApplication } from '@/features/system/client'

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
  const { t } = useI18n({ useScope: 'global' })
  let resources: NativeMenuResources | undefined
  let resourcesPromise: Promise<NativeMenuResources> | undefined
  let showing = false
  let disposed = false
  let renderedLocale: string | undefined

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
    const created: Array<{ close: () => Promise<void> }> = []

    /** 记录已创建资源，后续步骤失败时统一释放。 */
    async function retain<T extends { close: () => Promise<void> }>(
      pending: Promise<T>,
    ): Promise<T> {
      const resource = await pending
      created.push(resource)
      return resource
    }

    const createCheckItem = (options: CheckMenuItemOptions) => CheckMenuItem.new(options)
    try {
      const normalCover = await retain(
        createCheckItem({
          id: 'taskbar-toggle-normal-cover',
          text: t('taskbar.menu.normalCover'),
          action: () => runAction(() => toggleCover('normal'), '切换普通模式封面失败'),
        }),
      )
      const lyricsCover = await retain(
        createCheckItem({
          id: 'taskbar-toggle-lyrics-cover',
          text: t('taskbar.menu.lyricsCover'),
          action: () => runAction(() => toggleCover('lyrics'), '切换歌词模式封面失败'),
        }),
      )
      const lyrics = await retain(
        createCheckItem({
          id: 'taskbar-toggle-lyrics',
          text: t('taskbar.menu.lyrics'),
          action: () => runAction(toggleLyrics, '切换歌词失败'),
        }),
      )
      const spectrum = await retain(
        createCheckItem({
          id: 'taskbar-toggle-spectrum',
          text: t('taskbar.menu.spectrum'),
          action: () => runAction(toggleSpectrum, '切换频谱显示失败'),
        }),
      )
      const separator = await retain(PredefinedMenuItem.new({ item: 'Separator' }))
      const settings = await retain(
        MenuItem.new({
          id: 'taskbar-open-settings',
          text: t('taskbar.menu.settings'),
          action: () => runAction(openSettingsWindow, '从任务栏菜单打开设置失败'),
        }),
      )
      const restart = await retain(
        MenuItem.new({
          id: 'taskbar-restart-application',
          text: import.meta.env.PROD
            ? t('taskbar.menu.restart')
            : t('taskbar.menu.restartProductionOnly'),
          enabled: import.meta.env.PROD,
          action: () => runAction(restartApplication, '从任务栏菜单重启应用失败'),
        }),
      )
      const quit = await retain(
        PredefinedMenuItem.new({ item: 'Quit', text: t('taskbar.menu.quit') }),
      )
      const menu = await retain(
        Menu.new({
          items: [normalCover, lyricsCover, lyrics, spectrum, separator, settings, restart, quit],
        }),
      )
      renderedLocale = i18n.global.locale.value
      return {
        menu,
        normalCover,
        lyricsCover,
        lyrics,
        spectrum,
        separator,
        settings,
        restart,
        quit,
      }
    } catch (error) {
      await Promise.allSettled(created.map((resource) => resource.close()))
      throw error
    }
  }

  /** 获取当前窗口唯一的菜单实例，并合并并发初始化。 */
  async function getResources(): Promise<NativeMenuResources> {
    if (resources) return resources
    resourcesPromise ??= createResources()
    let created: NativeMenuResources
    try {
      created = await resourcesPromise
    } catch (error) {
      resourcesPromise = undefined
      throw error
    }
    if (disposed) {
      await closeResources(created)
      throw new Error('任务栏窗口已关闭')
    }
    resources = created
    resourcesPromise = undefined
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

  /** 每次弹出前刷新菜单文本，使运行期间切换语言无需重建原生资源。 */
  async function refreshText(current: NativeMenuResources) {
    const locale = i18n.global.locale.value
    if (locale === renderedLocale) return
    await Promise.all([
      current.normalCover.setText(t('taskbar.menu.normalCover')),
      current.lyricsCover.setText(t('taskbar.menu.lyricsCover')),
      current.lyrics.setText(t('taskbar.menu.lyrics')),
      current.spectrum.setText(t('taskbar.menu.spectrum')),
      current.settings.setText(t('taskbar.menu.settings')),
      current.restart.setText(
        import.meta.env.PROD ? t('taskbar.menu.restart') : t('taskbar.menu.restartProductionOnly'),
      ),
      current.quit.setText(t('taskbar.menu.quit')),
    ])
    renderedLocale = locale
  }

  /** 在鼠标当前位置显示 Windows 原生菜单；重复右键不会并行打开多个菜单。 */
  async function show() {
    if (showing || disposed) return
    showing = true
    try {
      const current = await getResources()
      await Promise.all([refreshCheckedState(current), refreshText(current)])
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
