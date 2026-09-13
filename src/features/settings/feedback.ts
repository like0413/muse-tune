import { toast } from 'vue-sonner'

/** 将未知异常收敛为适合展示的一行说明。 */
function getErrorDescription(error: unknown): string {
  if (error instanceof Error) return error.message
  return typeof error === 'string' ? error : String(error)
}

/** 统一记录并提示设置保存失败，避免各设置项静默失败。 */
export function notifySettingSaveFailed(label: string, error: unknown): void {
  console.error(`保存${label}失败`, error)
  toast.error('设置保存失败', { description: `${label}：${getErrorDescription(error)}` })
}
