import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart'

const AUTOSTART_CHANGED_EVENT = 'settings://autostart-changed'

/** 读取当前应用在 Windows 中真实的开机自启状态，不另存设置副本。 */
export function getAutostartEnabled(): Promise<boolean> {
  return isEnabled()
}

/** 通过官方插件修改自启动，成功后通知原生托盘与设置窗口重新读取。 */
export async function setAutostartEnabled(enabled: boolean): Promise<void> {
  if (enabled) await enable()
  else await disable()
  await emit(AUTOSTART_CHANGED_EVENT)
}

/** 订阅自启动变化通知；真实状态始终由官方插件查询。 */
export function listenAutostartChange(handler: () => void): Promise<UnlistenFn> {
  return listen(AUTOSTART_CHANGED_EVENT, handler)
}
