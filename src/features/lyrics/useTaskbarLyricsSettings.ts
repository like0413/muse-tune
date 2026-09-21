import { useEventState } from '@/features/ipc/useEventState'
import {
  DEFAULT_TASKBAR_LYRICS_SETTINGS,
  getTaskbarLyricsSettings,
  listenTaskbarLyricsSettingsChange,
} from '@/features/settings/lyrics'

/** 为单个任务栏窗口恢复并订阅歌词显示配置。 */
export function useTaskbarLyricsSettings() {
  const settings = useEventState(
    {
      read: getTaskbarLyricsSettings,
      subscribe: listenTaskbarLyricsSettingsChange,
      failureMessage: '初始化歌词显示配置失败',
    },
    { ...DEFAULT_TASKBAR_LYRICS_SETTINGS },
  )

  return { settings: readonly(settings) }
}
