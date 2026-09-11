import { invoke } from '@tauri-apps/api/core'
import { isEqual } from 'es-toolkit'

let cachedSystemFonts: readonly string[] = []
let refreshRequest: Promise<readonly string[]> | undefined

/** 返回内存中的字体快照，让选择器展开时无需等待系统枚举。 */
export function getCachedSystemFonts(): readonly string[] {
  return cachedSystemFonts
}

/** 后台刷新 Windows 字体；并发调用复用同一次枚举，内容未变化时保留原引用。 */
export function refreshSystemFonts(): Promise<readonly string[]> {
  refreshRequest ??= loadSystemFonts().finally(() => {
    refreshRequest = undefined
  })
  return refreshRequest
}

/** 调用后端枚举并规范字体族列表。 */
async function loadSystemFonts(): Promise<readonly string[]> {
  const fonts = await invoke<unknown>('list_system_fonts')
  const nextFonts = Array.isArray(fonts)
    ? fonts.filter((font): font is string => typeof font === 'string' && font.length > 0)
    : []
  if (!isEqual(cachedSystemFonts, nextFonts)) cachedSystemFonts = nextFonts
  return cachedSystemFonts
}
