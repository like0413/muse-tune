import { invoke } from '@tauri-apps/api/core'

export interface UpdateTrayPresentation {
  label: string | null
  tooltip: string | null
}

/** 同步原生托盘中的持久更新入口；不负责保存更新检测结果。 */
export function setUpdateTrayState(presentation: UpdateTrayPresentation): Promise<void> {
  return invoke('set_update_tray_state', { presentation })
}
