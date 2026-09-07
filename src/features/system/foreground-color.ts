import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

const SYSTEM_FOREGROUND_COLOR_CHANGED_EVENT = 'system://foreground-color-changed'

/** 读取 Windows 当前前景色。 */
export async function getSystemForegroundColor(): Promise<string> {
  return invoke<string>('get_system_foreground_color')
}

/** 监听 Windows 前景色变化。 */
export async function listenSystemForegroundColorChange(
  handler: (color: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(SYSTEM_FOREGROUND_COLOR_CHANGED_EVENT, ({ payload }) => handler(payload))
}
