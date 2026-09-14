/** 前后端共享的结构化 IPC 错误形状。 */
export interface IpcError {
  code: string
  message: string
  retryable: boolean
  context?: Record<string, unknown>
}

/** 判断未知值是否为可读取字段的普通对象。 */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

/** 判断未知值是否已经符合结构化 IPC 错误契约。 */
export function isIpcError(value: unknown): value is IpcError {
  return (
    isRecord(value) &&
    typeof value.code === 'string' &&
    typeof value.message === 'string' &&
    typeof value.retryable === 'boolean'
  )
}

/** 将 Tauri 拒绝值统一转换为前端可展示、可判断是否重试的错误。 */
export function normalizeIpcError(value: unknown, fallbackCode = 'ipc.unknown'): IpcError {
  if (isIpcError(value)) return value

  if (typeof value === 'string') {
    try {
      const parsed: unknown = JSON.parse(value)
      if (isIpcError(parsed)) return parsed
    } catch {
      // 普通字符串不是 JSON 错误对象，继续按消息处理。
    }
    return { code: fallbackCode, message: value, retryable: false }
  }

  if (value instanceof Error) {
    return { code: fallbackCode, message: value.message, retryable: false }
  }

  return { code: fallbackCode, message: String(value), retryable: false }
}
