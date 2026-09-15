import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'

import { translateGlobal } from '@/features/i18n'

/** 通过系统通知提示首次发现的新版本；权限被拒绝时保持静默。 */
export async function notifyUpdateAvailable(version: string): Promise<boolean> {
  let granted = await isPermissionGranted()
  if (!granted) granted = (await requestPermission()) === 'granted'
  if (!granted) return false

  sendNotification({
    title: translateGlobal('settings.about.update.notificationTitle'),
    body: translateGlobal('settings.about.update.notificationBody', { version }),
    autoCancel: true,
  })
  return true
}
