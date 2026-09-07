import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

const SYSTEM_ACCENT_COLOR_CHANGED_EVENT = 'system://accent-color-changed'

/** 读取 Windows 当前强调色。 */
export async function getSystemAccentColor(): Promise<string> {
  return invoke<string>('get_system_accent_color')
}

/** 监听 Windows 强调色变化。 */
export async function listenSystemAccentColorChange(
  handler: (color: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(SYSTEM_ACCENT_COLOR_CHANGED_EVENT, ({ payload }) => handler(payload))
}
