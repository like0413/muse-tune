import type { DownloadEvent } from '@tauri-apps/plugin-updater'
import { clamp } from 'es-toolkit'

/**
 * 创建一次下载的进度累加器。
 *
 * 返回的处理器接收 Tauri Updater 的原始事件，输出可直接绑定的百分比；
 * **内容长度未知时返回 `null`**，界面据此显示"不确定进度"，而不是伪造成 0%。
 * 每次 `Started` 都会重置累计字节，因此重试下载不需要新建累加器。
 */
export function createDownloadProgressTracker(): (event: DownloadEvent) => number | null {
  let downloadedBytes = 0
  let totalBytes: number | undefined

  return (event) => {
    if (event.event === 'Started') {
      downloadedBytes = 0
      totalBytes = event.data.contentLength
      return totalBytes ? 0 : null
    }
    if (event.event === 'Progress') {
      downloadedBytes += event.data.chunkLength
      return totalBytes ? clamp((downloadedBytes / totalBytes) * 100, 0, 100) : null
    }
    // Finished
    return totalBytes ? 100 : null
  }
}
