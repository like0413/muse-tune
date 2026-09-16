import { useThrottleFn } from '@vueuse/core'

import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  applyTaskbarAudioSpectrumSettings,
  DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS,
  getTaskbarAudioSpectrumSettings,
  isTaskbarSpectrumAlignment,
  normalizeTaskbarAudioSpectrumSettings,
  setTaskbarAudioSpectrumSettings,
  type TaskbarAudioSpectrumSettings,
} from '@/features/settings/audio-spectrum'

type SpectrumSliderKey =
  | 'barCount'
  | 'widthPercentage'
  | 'horizontalPosition'
  | 'sensitivity'
  | 'smoothing'

/** 管理任务栏频谱设置的预览、持久化和失败回滚。 */
export function useAudioSpectrumSetting() {
  const { t } = useI18n({ useScope: 'global' })
  const selectedSettings = shallowRef<TaskbarAudioSpectrumSettings>({
    ...DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS,
  })
  const committedSettings = shallowRef<TaskbarAudioSpectrumSettings>({
    ...DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS,
  })
  const settingsSaving = shallowRef(false)

  /** 恢复已保存的频谱配置。 */
  async function loadSettings() {
    try {
      const settings = await getTaskbarAudioSpectrumSettings()
      selectedSettings.value = settings
      committedSettings.value = { ...settings }
    } catch (error) {
      console.error('读取任务栏频谱配置失败', error)
    }
  }

  /** 限频广播滑块预览，避免高频跨窗口事件。 */
  const previewSettings = useThrottleFn(
    (settings: TaskbarAudioSpectrumSettings) => {
      applyTaskbarAudioSpectrumSettings(settings).catch((error) => {
        console.error('预览任务栏频谱配置失败', error)
      })
    },
    50,
    true,
    false,
  )

  /** 合并并持久化一次离散配置变更，失败时恢复最近成功值。 */
  async function updateSettings(patch: Partial<TaskbarAudioSpectrumSettings>) {
    if (settingsSaving.value) return
    const next = normalizeTaskbarAudioSpectrumSettings({ ...selectedSettings.value, ...patch })
    selectedSettings.value = next
    settingsSaving.value = true
    try {
      await setTaskbarAudioSpectrumSettings(next)
      committedSettings.value = { ...next }
    } catch (error) {
      selectedSettings.value = { ...committedSettings.value }
      notifySettingSaveFailed(t('settings.taskbar.spectrum.title'), error)
    } finally {
      settingsSaving.value = false
    }
  }

  /** 接收 Tabs 外部值并更新频谱垂直位置。 */
  function selectAlignment(value: string | number) {
    if (isTaskbarSpectrumAlignment(value)) void updateSettings({ alignment: value })
  }

  /** 规范单个滑块值并广播完整配置。 */
  function updateSliderPreview(key: SpectrumSliderKey, value: number | undefined) {
    if (settingsSaving.value || value === undefined) return
    const next = normalizeTaskbarAudioSpectrumSettings({
      ...selectedSettings.value,
      [key]: value,
    })
    selectedSettings.value = next
    void previewSettings(next)
  }

  /** 更新频谱条数草稿并实时预览。 */
  function updateBarCount(values: number[] | undefined) {
    updateSliderPreview('barCount', values?.[0])
  }

  /** 更新频谱相对 bar 宽度的百分比并实时预览。 */
  function updateWidthPercentage(values: number[] | undefined) {
    updateSliderPreview('widthPercentage', values?.[0])
  }

  /** 更新频谱水平位置草稿并实时预览。 */
  function updateHorizontalPosition(values: number[] | undefined) {
    updateSliderPreview('horizontalPosition', values?.[0])
  }

  /** 更新频谱输入增益草稿并实时预览。 */
  function updateSensitivity(values: number[] | undefined) {
    updateSliderPreview('sensitivity', values?.[0])
  }

  /** 更新频谱动态平滑草稿并实时预览。 */
  function updateSmoothing(values: number[] | undefined) {
    updateSliderPreview('smoothing', values?.[0])
  }

  /** 在滑块交互结束后持久化当前完整配置。 */
  function commitSlider() {
    void updateSettings(selectedSettings.value)
  }

  onMounted(() => void loadSettings())

  return {
    selectedSettings: readonly(selectedSettings),
    settingsSaving: readonly(settingsSaving),
    updateSettings,
    selectAlignment,
    updateBarCount,
    updateWidthPercentage,
    updateHorizontalPosition,
    updateSensitivity,
    updateSmoothing,
    commitSlider,
  }
}
