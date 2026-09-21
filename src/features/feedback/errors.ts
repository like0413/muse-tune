import { toast } from 'vue-sonner'

import { translateGlobal } from '@/features/i18n'
import { isIpcError } from '@/features/ipc/errors'
import { logError } from '@/features/logging'

/** 将未知异常收敛为可展示的一行文案，避免直接显示对象序列化结果。 */
export function getErrorMessage(
  error: unknown,
  fallback = translateGlobal('feedback.defaultError'),
): string {
  if (isIpcError(error) && error.message.trim()) return error.message
  if (error instanceof Error && error.message.trim()) return error.message
  if (typeof error === 'string' && error.trim()) return error
  return fallback
}

/** 记录无需用户立即处理的后台失败，不用 toast 打断正常操作。 */
export function reportBackgroundFailure(context: string, error: unknown): void {
  console.error(context, error)
  // 控制台在正式构建里看不到，同一份内容必须落盘，这是前台失败唯一的追溯途径。
  logError(context, error)
}

/** 重复失败的最短记录间隔：高频路径不限频会以每秒数十条的速度填满日志预算。 */
const REPEATED_FAILURE_INTERVAL_MS = 5 * 60 * 1_000
const lastReportedAt = new Map<string, number>()

/**
 * 记录会按用户操作频率持续复现的失败。
 *
 * 与 `reportBackgroundFailure` 走同样的两条通道，但按调用点限频：第一次立即可见，
 * 之后每 5 分钟最多一条，既不刷满日志预算，也不丢掉“问题还在持续”的信号。
 */
export function reportRepeatedFailure(context: string, error: unknown): void {
  const now = Date.now()
  const previous = lastReportedAt.get(context)
  if (previous !== undefined && now - previous < REPEATED_FAILURE_INTERVAL_MS) return
  lastReportedAt.set(context, now)
  reportBackgroundFailure(context, error)
}

/** 提示当前用户操作失败，同时保留原始异常供开发诊断。 */
export function notifyActionFailed(
  title: string,
  error: unknown,
  options: {
    context?: string
    descriptionPrefix?: string
    fallbackMessage?: string
  } = {},
): void {
  const context = options.context ?? title
  reportBackgroundFailure(context, error)

  const message = getErrorMessage(error, options.fallbackMessage)
  const description = options.descriptionPrefix
    ? `${options.descriptionPrefix}：${message}`
    : message
  toast.error(title, { description })
}

/** 统一提示设置保存失败，设置读取失败仍按页面重要性单独处理。 */
export function notifySettingSaveFailed(label: string, error: unknown): void {
  notifyActionFailed(translateGlobal('feedback.settingSaveFailed'), error, {
    context: `保存${label}失败`,
    descriptionPrefix: label,
    fallbackMessage: translateGlobal('feedback.settingSaveFallback'),
  })
}
