import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'
import { clamp } from 'es-toolkit'

import { SETTINGS_SCHEMA_VERSIONS } from './storage/schema-versions'
import { getVersionedSetting, setVersionedSetting } from './storage/versioned-setting'

const TASKBAR_AUDIO_SPECTRUM_KEY = 'taskbar.audioSpectrum'
const TASKBAR_AUDIO_SPECTRUM_CHANGED_EVENT = 'settings://taskbar-audio-spectrum-changed'

export const TASKBAR_SPECTRUM_BAR_COUNT_MIN = 8
export const TASKBAR_SPECTRUM_BAR_COUNT_MAX = 48
export const TASKBAR_SPECTRUM_WIDTH_PERCENTAGE_MIN = 20
export const TASKBAR_SPECTRUM_WIDTH_PERCENTAGE_MAX = 100
export const TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MIN = 0
export const TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MAX = 100
export const TASKBAR_SPECTRUM_SENSITIVITY_MIN = 50
export const TASKBAR_SPECTRUM_SENSITIVITY_MAX = 200
export const TASKBAR_SPECTRUM_SMOOTHING_MIN = 0
export const TASKBAR_SPECTRUM_SMOOTHING_MAX = 90

const TASKBAR_SPECTRUM_ALIGNMENTS = ['center', 'bottom'] as const

export type TaskbarSpectrumAlignment = (typeof TASKBAR_SPECTRUM_ALIGNMENTS)[number]

export interface TaskbarAudioSpectrumSettings {
  visible: boolean
  barCount: number
  widthPercentage: number
  alignment: TaskbarSpectrumAlignment
  horizontalPosition: number
  sensitivity: number
  smoothing: number
}

export const DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS: TaskbarAudioSpectrumSettings = {
  visible: true,
  barCount: 24,
  widthPercentage: 72,
  alignment: 'bottom',
  horizontalPosition: 50,
  sensitivity: 100,
  smoothing: 55,
}

/** 判断外部值是否为支持的频谱对齐方式。 */
export function isTaskbarSpectrumAlignment(value: unknown): value is TaskbarSpectrumAlignment {
  return (
    typeof value === 'string' &&
    TASKBAR_SPECTRUM_ALIGNMENTS.some((alignment) => alignment === value)
  )
}

/** 将外部值规范为完整、安全的频谱配置。 */
export function normalizeTaskbarAudioSpectrumSettings(
  value: unknown,
): TaskbarAudioSpectrumSettings {
  const candidate = typeof value === 'object' && value !== null ? value : {}
  const record = candidate as Partial<Record<keyof TaskbarAudioSpectrumSettings, unknown>>
  return {
    visible:
      typeof record.visible === 'boolean'
        ? record.visible
        : DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS.visible,
    barCount: normalizeInteger(
      record.barCount,
      TASKBAR_SPECTRUM_BAR_COUNT_MIN,
      TASKBAR_SPECTRUM_BAR_COUNT_MAX,
      DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS.barCount,
    ),
    widthPercentage: normalizeInteger(
      record.widthPercentage,
      TASKBAR_SPECTRUM_WIDTH_PERCENTAGE_MIN,
      TASKBAR_SPECTRUM_WIDTH_PERCENTAGE_MAX,
      DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS.widthPercentage,
    ),
    alignment: isTaskbarSpectrumAlignment(record.alignment)
      ? record.alignment
      : DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS.alignment,
    horizontalPosition: normalizeInteger(
      record.horizontalPosition,
      TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MIN,
      TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MAX,
      DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS.horizontalPosition,
    ),
    sensitivity: normalizeInteger(
      record.sensitivity,
      TASKBAR_SPECTRUM_SENSITIVITY_MIN,
      TASKBAR_SPECTRUM_SENSITIVITY_MAX,
      DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS.sensitivity,
    ),
    smoothing: normalizeInteger(
      record.smoothing,
      TASKBAR_SPECTRUM_SMOOTHING_MIN,
      TASKBAR_SPECTRUM_SMOOTHING_MAX,
      DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS.smoothing,
    ),
  }
}

const audioSpectrumStorage = {
  key: TASKBAR_AUDIO_SPECTRUM_KEY,
  version: SETTINGS_SCHEMA_VERSIONS.taskbar.audioSpectrum,
  defaultValue: DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS,
  normalize: normalizeTaskbarAudioSpectrumSettings,
}

/** 读取频谱配置；版本不兼容时仅恢复此项默认值。 */
export async function getTaskbarAudioSpectrumSettings(): Promise<TaskbarAudioSpectrumSettings> {
  return getVersionedSetting(audioSpectrumStorage)
}

/** 仅广播频谱配置，用于设置窗口拖动时的轻量实时预览。 */
export async function applyTaskbarAudioSpectrumSettings(
  settings: TaskbarAudioSpectrumSettings,
): Promise<void> {
  await emit(TASKBAR_AUDIO_SPECTRUM_CHANGED_EVENT, normalizeTaskbarAudioSpectrumSettings(settings))
}

/** 持久化完整频谱配置，并通知任务栏立即更新。 */
export async function setTaskbarAudioSpectrumSettings(
  settings: TaskbarAudioSpectrumSettings,
): Promise<void> {
  const saved = await setVersionedSetting(audioSpectrumStorage, settings)
  await emit(TASKBAR_AUDIO_SPECTRUM_CHANGED_EVENT, saved)
}

/** 监听设置窗口发出的频谱配置变化。 */
export async function listenTaskbarAudioSpectrumSettingsChange(
  handler: (settings: TaskbarAudioSpectrumSettings) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_AUDIO_SPECTRUM_CHANGED_EVENT, ({ payload }) => {
    handler(normalizeTaskbarAudioSpectrumSettings(payload))
  })
}

/** 将单个数值约束为指定范围内的整数。 */
function normalizeInteger(value: unknown, min: number, max: number, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.round(clamp(value, min, max))
    : fallback
}
