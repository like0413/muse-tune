import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import { SYSTEM_ACCENT_COLOR_CHANGED_EVENT } from './client'

export { getSystemAccentColor } from './client'

/** 监听 Windows 强调色变化。 */
export async function listenSystemAccentColorChange(
  handler: (color: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(SYSTEM_ACCENT_COLOR_CHANGED_EVENT, ({ payload }) => handler(payload))
}
