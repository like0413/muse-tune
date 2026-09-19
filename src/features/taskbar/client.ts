import { invoke } from '@tauri-apps/api/core'

import type {
  TaskbarDisplay,
  TaskbarOverlapPriority,
  TaskbarPlacement,
  TrayMenuPresentation,
} from './contracts'

export const TRAY_MENU_ACTION_EVENT = 'tray://taskbar-menu-action'

/** 读取系统任务栏显示器列表。 */
export function listTaskbarDisplays(): Promise<TaskbarDisplay[]> {
  return invoke<TaskbarDisplay[]>('list_taskbar_displays')
}

/** 设置任务栏目标显示器。 */
export function setTaskbarDisplayTarget(target: string): Promise<void> {
  return invoke('set_taskbar_display_target', { target })
}

/** 设置任务栏内容可见性。 */
export function setTaskbarContentVisibility(visible: boolean): Promise<void> {
  return invoke('set_taskbar_content_visibility', { visible })
}

/** 设置任务栏与系统区域重叠优先级。 */
export function setTaskbarOverlapPriority(priority: TaskbarOverlapPriority): Promise<void> {
  return invoke('set_taskbar_overlap_priority', { priority })
}

/** 设置任务栏布局位置。 */
export function setTaskbarPlacement(placement: TaskbarPlacement): Promise<void> {
  return invoke('set_taskbar_placement', { placement })
}

/** 设置任务栏宽度。 */
export function setTaskbarWidth(width: number): Promise<void> {
  return invoke('set_taskbar_width', { width })
}

/** 同步托盘菜单的文案与勾选状态。 */
export function setTrayMenuState(presentation: TrayMenuPresentation): Promise<void> {
  return invoke('set_tray_menu_state', { presentation })
}

/** 显示原生音量悬浮窗。 */
export function showVolumePopup(anchorCenterX: number, themeColor: string): Promise<void> {
  return invoke('show_volume_popup', { anchorCenterX, themeColor })
}

/** 按代次隐藏原生音量悬浮窗。 */
export function hideVolumePopup(generation: number): Promise<void> {
  return invoke('hide_volume_popup', { generation })
}
