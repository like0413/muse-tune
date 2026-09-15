import { i18n, translateGlobal } from '@/features/i18n'
import { getApplicationLocaleTag } from '@/features/i18n/locales'

const sizeFormatters = new Map<string, Intl.NumberFormat>()

/** 按语言复用数字格式器，避免数据卡片每次渲染重复构造 Intl 对象。 */
function getSizeFormatter(): Intl.NumberFormat {
  const locale = getApplicationLocaleTag(i18n.global.locale.value)
  const cached = sizeFormatters.get(locale)
  if (cached) return cached
  const formatter = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 })
  sizeFormatters.set(locale, formatter)
  return formatter
}

/** 将字节数统一格式化为设置页使用的 KB 或 MB。 */
export function formatBytes(bytes: number | null): string {
  if (bytes === null) return translateGlobal('common.unavailable')
  const formatter = getSizeFormatter()
  if (bytes < 1024 ** 2) return `${formatter.format(bytes / 1024)} KB`
  return `${formatter.format(bytes / 1024 ** 2)} MB`
}
