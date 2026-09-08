import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'
import { clamp } from 'es-toolkit'

import { settingsStore } from './store'

const TASKBAR_AUDIO_SPECTRUM_KEY = 'taskbar.audioSpectrum'
const TASKBAR_AUDIO_SPECTRUM_CHANGED_EVENT = 'settings://taskbar-audio-spectrum-changed'

export const TASKBAR_SPECTRUM_BAR_COUNT_MIN = 8
export const TASKBAR_SPECTRUM_BAR_COUNT_MAX = 48
export const TASKBAR_SPECTRUM_MAX_WIDTH_MIN = 80
export const TASKBAR_SPECTRUM_MAX_WIDTH_MAX = 360
export const TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MIN = 0
export const TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MAX = 100

const TASKBAR_SPECTRUM_ALIGNMENTS = ['center', 'bottom'] as const

export type TaskbarSpectrumAlignment = (typeof TASKBAR_SPECTRUM_ALIGNMENTS)[number]

export interface TaskbarAudioSpectrumSettings {
  visible: boolean
  barCount: number
  maxWidth: number
  alignment: TaskbarSpectrumAlignment
  horizontalPosition: number
}

export const DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS: TaskbarAudioSpectrumSettings = {
  visible: true,
  barCount: 24,
  maxWidth: 260,
  alignment: 'bottom',
  horizontalPosition: 50,
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
  const record = candidate as Partial<Record<keyof TaskbarAudioSpectrumSettings, unknown>> & {
    horizontalAlignment?: unknown
  }
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
    maxWidth: normalizeInteger(
      record.maxWidth,
      TASKBAR_SPECTRUM_MAX_WIDTH_MIN,
      TASKBAR_SPECTRUM_MAX_WIDTH_MAX,
      DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS.maxWidth,
    ),
    alignment: isTaskbarSpectrumAlignment(record.alignment)
      ? record.alignment
      : DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS.alignment,
    horizontalPosition: normalizeInteger(
      record.horizontalPosition,
      TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MIN,
      TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MAX,
      legacyHorizontalPosition(record.horizontalAlignment),
    ),
  }
}

/** 把上一版三档水平对齐设置迁移为连续位置。 */
function legacyHorizontalPosition(value: unknown): number {
  if (value === 'left') return TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MIN
  if (value === 'right') return TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MAX
  return DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS.horizontalPosition
}

/** 读取频谱配置，缺失或损坏字段分别回退默认值。 */
export async function getTaskbarAudioSpectrumSettings(): Promise<TaskbarAudioSpectrumSettings> {
  return normalizeTaskbarAudioSpectrumSettings(
    await settingsStore.get<unknown>(TASKBAR_AUDIO_SPECTRUM_KEY),
  )
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
  const normalized = normalizeTaskbarAudioSpectrumSettings(settings)
  await settingsStore.set(TASKBAR_AUDIO_SPECTRUM_KEY, normalized)
  await emit(TASKBAR_AUDIO_SPECTRUM_CHANGED_EVENT, normalized)
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
