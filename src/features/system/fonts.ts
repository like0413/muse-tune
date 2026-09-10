import { invoke } from '@tauri-apps/api/core'

/** 读取 Windows 当前安装的字体族；每次打开设置页重新读取以发现新安装字体。 */
export async function listSystemFonts(): Promise<string[]> {
  const fonts = await invoke<unknown>('list_system_fonts')
  if (!Array.isArray(fonts)) return []
  return fonts.filter((font): font is string => typeof font === 'string' && font.length > 0)
}
