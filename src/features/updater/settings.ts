import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { DEFAULT_UPDATE_CHECK_RESULT } from '@/features/settings/defaults'
import { SETTINGS_SCHEMA_VERSIONS } from '@/features/settings/storage/schema-versions'
import {
  loadVersionedSetting,
  setVersionedSetting,
} from '@/features/settings/storage/versioned-setting'

const UPDATE_CHECK_RESULT_KEY = 'application.update-check-result'
const UPDATE_CHECK_RESULT_CHANGED_EVENT = 'updater://result-changed'

export interface UpdateCheckResult {
  /** 最近一次成功检查的时间。 */
  checkedAt: number
  /** 最近一次检查尝试的时间，包含失败。 */
  attemptedAt: number
  availableVersion: string | null
  /** 连续失败次数，用于持久化重试间隔。 */
  consecutiveFailures: number
}

/** 只接受有效的时间戳，缺失、NaN 或非正数一律回退为 0。 */
function normalizeTimestamp(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : 0
}

function normalizeUpdateCheckResult(value: unknown): UpdateCheckResult {
  const candidate =
    typeof value === 'object' && value !== null
      ? (value as Partial<Record<keyof UpdateCheckResult, unknown>>)
      : {}
  return {
    checkedAt: normalizeTimestamp(candidate.checkedAt),
    // 旧数据没有这一项：回退为 0 表示“还没尝试过”，下次调度立即检测一次。
    attemptedAt: normalizeTimestamp(candidate.attemptedAt),
    consecutiveFailures: Math.floor(normalizeTimestamp(candidate.consecutiveFailures)),
    availableVersion:
      typeof candidate.availableVersion === 'string' && candidate.availableVersion.length > 0
        ? candidate.availableVersion
        : null,
  }
}

const updateCheckResultOptions = {
  key: UPDATE_CHECK_RESULT_KEY,
  version: SETTINGS_SCHEMA_VERSIONS.application.updateCheckResult,
  defaultValue: DEFAULT_UPDATE_CHECK_RESULT,
  normalize: normalizeUpdateCheckResult,
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

/** 监听其他窗口完成的更新检测。 */
export function listenUpdateCheckResultChange(
  handler: (result: UpdateCheckResult) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(UPDATE_CHECK_RESULT_CHANGED_EVENT, ({ payload }) => {
    handler(normalizeUpdateCheckResult(payload))
  })
}
