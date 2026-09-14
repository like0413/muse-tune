import { invoke } from '@tauri-apps/api/core'

/** 请求后端打开或聚焦设置窗口。 */
export function openSettingsWindow(): Promise<void> {
  return invoke('open_settings_window')
}
