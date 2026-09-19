import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import {
  DEFAULT_AUTOMATIC_UPDATE_CHECK,
  DEFAULT_UPDATE_CHECK_FREQUENCY,
  DEFAULT_UPDATE_CHECK_RESULT,
} from '@/features/settings/defaults'
import { SETTINGS_SCHEMA_VERSIONS } from '@/features/settings/storage/schema-versions'
import {
  loadVersionedSetting,
  setVersionedSetting,
} from '@/features/settings/storage/versioned-setting'

const AUTOMATIC_UPDATE_CHECK_KEY = 'application.automatic-update-check'
const UPDATE_CHECK_FREQUENCY_KEY = 'application.update-check-frequency'
const UPDATE_CHECK_RESULT_KEY = 'application.update-check-result'
const UPDATE_CHECK_PREFERENCES_CHANGED_EVENT = 'updater://preferences-changed'
const UPDATE_CHECK_RESULT_CHANGED_EVENT = 'updater://result-changed'

const UPDATE_CHECK_FREQUENCIES = ['daily', 'weekly', 'monthly'] as const
export type UpdateCheckFrequency = (typeof UPDATE_CHECK_FREQUENCIES)[number]

export interface UpdateCheckResult {
  checkedAt: number
  availableVersion: string | null
}

export { DEFAULT_UPDATE_CHECK_FREQUENCY }

const UPDATE_CHECK_INTERVALS: Record<UpdateCheckFrequency, number> = {
  daily: 24 * 60 * 60 * 1_000,
  weekly: 7 * 24 * 60 * 60 * 1_000,
  monthly: 30 * 24 * 60 * 60 * 1_000,
}

function normalizeAutomaticUpdateCheck(value: unknown): boolean {
  return typeof value === 'boolean' ? value : DEFAULT_AUTOMATIC_UPDATE_CHECK
}

/** 判断持久化值是否为受支持的自动更新检查频率。 */
export function isUpdateCheckFrequency(value: unknown): value is UpdateCheckFrequency {
  return UPDATE_CHECK_FREQUENCIES.some((frequency) => frequency === value)
}

/** 将损坏或旧版频率设置回退为默认值。 */
function normalizeUpdateCheckFrequency(value: unknown): UpdateCheckFrequency {
  return isUpdateCheckFrequency(value) ? value : DEFAULT_UPDATE_CHECK_FREQUENCY
}

function normalizeUpdateCheckResult(value: unknown): UpdateCheckResult {
  const candidate =
    typeof value === 'object' && value !== null
      ? (value as Partial<Record<keyof UpdateCheckResult, unknown>>)
      : {}
  return {
    checkedAt:
      typeof candidate.checkedAt === 'number' &&
      Number.isFinite(candidate.checkedAt) &&
      candidate.checkedAt > 0
        ? candidate.checkedAt
        : 0,
    availableVersion:
      typeof candidate.availableVersion === 'string' && candidate.availableVersion.length > 0
        ? candidate.availableVersion
        : null,
  }
}

const automaticUpdateCheckOptions = {
  key: AUTOMATIC_UPDATE_CHECK_KEY,
  version: SETTINGS_SCHEMA_VERSIONS.application.automaticUpdateCheck,
  defaultValue: DEFAULT_AUTOMATIC_UPDATE_CHECK,
  normalize: normalizeAutomaticUpdateCheck,
}

const updateCheckFrequencyOptions = {
  key: UPDATE_CHECK_FREQUENCY_KEY,
  version: SETTINGS_SCHEMA_VERSIONS.application.updateCheckFrequency,
  defaultValue: DEFAULT_UPDATE_CHECK_FREQUENCY,
  normalize: normalizeUpdateCheckFrequency,
}

const updateCheckResultOptions = {
  key: UPDATE_CHECK_RESULT_KEY,
  version: SETTINGS_SCHEMA_VERSIONS.application.updateCheckResult,
  defaultValue: DEFAULT_UPDATE_CHECK_RESULT,
  normalize: normalizeUpdateCheckResult,
}

/** 读取是否启用应用级自动更新检测。 */
export function getAutomaticUpdateCheck(): Promise<boolean> {
  return loadVersionedSetting(automaticUpdateCheckOptions)
}

/** 保存自动检测开关，并通知运行中的任务栏更新调度器。 */
export async function setAutomaticUpdateCheck(value: boolean): Promise<boolean> {
  const normalized = await setVersionedSetting(automaticUpdateCheckOptions, value)
  await emit(UPDATE_CHECK_PREFERENCES_CHANGED_EVENT)
  return normalized
}

/** 读取自动更新检测频率。 */
export function getUpdateCheckFrequency(): Promise<UpdateCheckFrequency> {
  return loadVersionedSetting(updateCheckFrequencyOptions)
}

/** 保存自动检测频率，并通知运行中的任务栏更新调度器。 */
export async function setUpdateCheckFrequency(value: unknown): Promise<UpdateCheckFrequency> {
  const normalized = await setVersionedSetting(updateCheckFrequencyOptions, value)
  await emit(UPDATE_CHECK_PREFERENCES_CHANGED_EVENT)
  return normalized
}

/** 返回指定频率对应的毫秒间隔。 */
export function getUpdateCheckInterval(frequency: UpdateCheckFrequency): number {
  return UPDATE_CHECK_INTERVALS[frequency]
}

/** 读取最近一次成功检测的结果。 */
export function getUpdateCheckResult(): Promise<UpdateCheckResult> {
  return loadVersionedSetting(updateCheckResultOptions)
}

/** 提交成功检测结果，并广播给已打开的设置窗口。 */
export async function setUpdateCheckResult(result: UpdateCheckResult): Promise<UpdateCheckResult> {
  const normalized = await setVersionedSetting(updateCheckResultOptions, result)
  await emit(UPDATE_CHECK_RESULT_CHANGED_EVENT, normalized)
  return normalized
}

/** 监听自动检测策略变化。 */
export function listenUpdateCheckPreferencesChange(handler: () => void): Promise<UnlistenFn> {
  return listen(UPDATE_CHECK_PREFERENCES_CHANGED_EVENT, handler)
}

/** 监听其他窗口完成的更新检测。 */
export function listenUpdateCheckResultChange(
  handler: (result: UpdateCheckResult) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(UPDATE_CHECK_RESULT_CHANGED_EVENT, ({ payload }) => {
    handler(normalizeUpdateCheckResult(payload))
  })
}
