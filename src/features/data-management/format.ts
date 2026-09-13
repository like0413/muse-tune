const sizeNumberFormatter = new Intl.NumberFormat('zh-CN', {
  maximumFractionDigits: 1,
})

/** 将字节数统一格式化为设置页使用的 KB 或 MB。 */
export function formatBytes(bytes: number | null): string {
  if (bytes === null) return '暂无'
  if (bytes < 1024 ** 2) return `${sizeNumberFormatter.format(bytes / 1024)} KB`
  return `${sizeNumberFormatter.format(bytes / 1024 ** 2)} MB`
}
