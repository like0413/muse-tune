import { toast } from 'vue-sonner'

import { isIpcError } from '@/features/ipc/errors'

const DEFAULT_ERROR_MESSAGE = '操作未完成，请稍后重试'

/** 将未知异常收敛为可展示的一行文案，避免直接显示对象序列化结果。 */
export function getErrorMessage(error: unknown, fallback = DEFAULT_ERROR_MESSAGE): string {
  if (isIpcError(error) && error.message.trim()) return error.message
  if (error instanceof Error && error.message.trim()) return error.message
  if (typeof error === 'string' && error.trim()) return error
  return fallback
}

/** 记录无需用户立即处理的后台失败，不用 toast 打断正常操作。 */
export function reportBackgroundFailure(context: string, error: unknown): void {
  console.error(context, error)
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
  notifyActionFailed('设置保存失败', error, {
    context: `保存${label}失败`,
    descriptionPrefix: label,
    fallbackMessage: '未能保存此项设置',
  })
}
