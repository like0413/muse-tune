/** Windows 任务栏对应的显示器摘要。 */
export interface TaskbarDisplay {
  id: string
  label: string
  width: number
  height: number
  isPrimary: boolean
}

/** 原生任务栏支持的播放器布局位置。 */
export const TASKBAR_PLACEMENTS = ['auto', 'left', 'right'] as const
/** 原生任务栏支持的遮挡优先级。 */
export const TASKBAR_OVERLAP_PRIORITIES = ['bar', 'taskbar'] as const

export type TaskbarPlacement = (typeof TASKBAR_PLACEMENTS)[number]
export type TaskbarOverlapPriority = (typeof TASKBAR_OVERLAP_PRIORITIES)[number]
