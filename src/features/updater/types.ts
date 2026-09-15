/** 关于页可展示的更新检查状态。 */
export type UpdateStatus = 'idle' | 'latest' | 'detected' | 'available' | 'error'

/** 从原生更新资源投影出的只读展示数据。 */
export interface AvailableUpdateView {
  currentVersion: string
  version: string
  date: string | null
}
