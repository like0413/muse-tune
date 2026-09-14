import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'

import { SYSTEM_FOREGROUND_COLOR_CHANGED_EVENT } from './client'

export { getSystemForegroundColor } from './client'

/** 监听 Windows 前景色变化。 */
export async function listenSystemForegroundColorChange(
  handler: (color: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(SYSTEM_FOREGROUND_COLOR_CHANGED_EVENT, ({ payload }) => handler(payload))
}
