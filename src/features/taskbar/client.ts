import { invoke } from '@tauri-apps/api/core'

import type {
  TaskbarDisplay,
  TaskbarOverlapPriority,
  TaskbarPlacement,
  TaskbarWidthMode,
  TrayMenuPresentation,
} from './contracts'

export const TRAY_MENU_ACTION_EVENT = 'tray://taskbar-menu-action'

export function listTaskbarDisplays(): Promise<TaskbarDisplay[]> {
  return invoke<TaskbarDisplay[]>('list_taskbar_displays')
}

export function setTaskbarDisplayTarget(target: string): Promise<void> {
  return invoke('set_taskbar_display_target', { target })
}

export function setTaskbarContentVisibility(visible: boolean): Promise<void> {
  return invoke('set_taskbar_content_visibility', { visible })
}

export function setTaskbarOverlapPriority(priority: TaskbarOverlapPriority): Promise<void> {
  return invoke('set_taskbar_overlap_priority', { priority })
}

export function setTaskbarPlacement(placement: TaskbarPlacement): Promise<void> {
  return invoke('set_taskbar_placement', { placement })
}

/** 将所有任务栏窗口相对默认位置水平平移指定 DIP。 */
export function setTaskbarHorizontalOffset(offset: number): Promise<void> {
  return invoke('set_taskbar_horizontal_offset', { offset })
}

export function setTaskbarWidth(width: number, mode: TaskbarWidthMode): Promise<void> {
  return invoke('set_taskbar_width', { width, mode })
}

export function setTrayMenuState(presentation: TrayMenuPresentation): Promise<void> {
  return invoke('set_tray_menu_state', { presentation })
}
