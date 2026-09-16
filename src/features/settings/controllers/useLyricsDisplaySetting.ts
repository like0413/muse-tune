import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_LYRICS_SETTINGS,
  getTaskbarLyricsSettings,
  normalizeTaskbarLyricsSettings,
  setTaskbarLyricsSettings,
  type TaskbarLyricsSettings,
} from '@/features/settings/lyrics'

/** 管理歌词显示设置的草稿、持久化和失败回滚。 */
export function useLyricsDisplaySetting() {
  const { t } = useI18n({ useScope: 'global' })
  const selectedSettings = shallowRef<TaskbarLyricsSettings>({
    ...DEFAULT_TASKBAR_LYRICS_SETTINGS,
  })
  const committedSettings = shallowRef<TaskbarLyricsSettings>({
    ...DEFAULT_TASKBAR_LYRICS_SETTINGS,
  })
  const settingsSaving = shallowRef(false)

  /** 恢复已保存的歌词显示配置。 */
  async function loadSettings() {
    try {
      const settings = await getTaskbarLyricsSettings()
      selectedSettings.value = settings
      committedSettings.value = { ...settings }
    } catch (error) {
      console.error('读取歌词显示配置失败', error)
    }
  }

  /** 合并并保存一次配置变更，失败时恢复最近成功状态。 */
  async function updateSettings(patch: Partial<TaskbarLyricsSettings>) {
    if (settingsSaving.value) return
    const next = normalizeTaskbarLyricsSettings({ ...selectedSettings.value, ...patch })
    selectedSettings.value = next
    settingsSaving.value = true
    try {
      await setTaskbarLyricsSettings(next)
      committedSettings.value = { ...next }
    } catch (error) {
      selectedSettings.value = { ...committedSettings.value }
      notifySettingSaveFailed(t('settings.taskbar.lyrics.title'), error)
    } finally {
      settingsSaving.value = false
    }
  }

  /** 拖动歌词时间偏移时只更新草稿。 */
  function previewTimingOffset(values: number[] | undefined) {
    const timingOffsetMs = values?.[0]
    if (timingOffsetMs === undefined) return
    selectedSettings.value = normalizeTaskbarLyricsSettings({
      ...selectedSettings.value,
      timingOffsetMs,
    })
  }

  /** 释放滑块后持久化歌词时间偏移。 */
  function commitTimingOffset(values: number[] | undefined) {
    const timingOffsetMs = values?.[0]
    if (timingOffsetMs !== undefined) void updateSettings({ timingOffsetMs })
  }

  /** 拖动字号滑块时只更新页面草稿，避免连续写入设置文件。 */
  function previewFontSize(values: number[] | undefined) {
    const fontSize = values?.[0]
    if (fontSize === undefined) return
    selectedSettings.value = normalizeTaskbarLyricsSettings({
      ...selectedSettings.value,
      fontSize,
    })
  }

  /** 滑块释放后持久化最终字号。 */
  function commitFontSize(values: number[] | undefined) {
    const fontSize = values?.[0]
    if (fontSize !== undefined) void updateSettings({ fontSize })
  }

  onMounted(() => void loadSettings())

  return {
    selectedSettings: readonly(selectedSettings),
    settingsSaving: readonly(settingsSaving),
    updateSettings,
    previewTimingOffset,
    commitTimingOffset,
    previewFontSize,
    commitFontSize,
  }
}
