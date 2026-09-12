import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'

/** 通过系统通知提示首次发现的新版本；权限被拒绝时保持静默。 */
export async function notifyUpdateAvailable(version: string): Promise<boolean> {
  let granted = await isPermissionGranted()
  if (!granted) granted = (await requestPermission()) === 'granted'
  if (!granted) return false

  sendNotification({
    title: 'Muse Tune 有新版本',
    body: `版本 ${version} 已发布，可前往“设置 → 关于”下载并安装。`,
    autoCancel: true,
  })
  return true
}
