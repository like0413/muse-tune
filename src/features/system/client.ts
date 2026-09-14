import { invoke } from '@tauri-apps/api/core'

export const SYSTEM_ACCENT_COLOR_CHANGED_EVENT = 'system://accent-color-changed'
export const SYSTEM_FOREGROUND_COLOR_CHANGED_EVENT = 'system://foreground-color-changed'

/** 读取 Windows 系统强调色。 */
export function getSystemAccentColor(): Promise<string> {
  return invoke<string>('get_system_accent_color')
}

/** 读取 Windows 前景色。 */
export function getSystemForegroundColor(): Promise<string> {
  return invoke<string>('get_system_foreground_color')
}

/** 读取系统可用字体列表。 */
export function listSystemFonts(): Promise<string[]> {
  return invoke<string[]>('list_system_fonts')
}

/** 请求应用按正常生命周期重启。 */
export function restartApplication(): Promise<void> {
  return invoke('restart_application')
}
