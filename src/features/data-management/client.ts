import { invoke } from '@tauri-apps/api/core'

import { normalizeIpcError } from '@/features/ipc/errors'

import type { DataDirectoryKind, DataOverview } from './types'

/** 执行 data command，并将 Tauri 的未知拒绝值收敛为统一错误。 */
async function request<T>(operation: string, action: () => Promise<T>): Promise<T> {
  try {
    return await action()
  } catch (error) {
    throw normalizeIpcError(error, `data.${operation}`)
  }
}

/** 读取应用管理的数据目录与占用摘要。 */
export function getDataOverview(): Promise<DataOverview> {
  return request('get-overview', () => invoke<DataOverview>('get_data_overview'))
}

/** 使用系统文件管理器打开指定应用数据目录。 */
export function openDataDirectory(kind: DataDirectoryKind): Promise<void> {
  return request('open-directory', () => invoke('open_data_directory', { kind }))
}

/** 清空歌词缓存并返回最新占用。 */
export function clearLyricsCache(): Promise<DataOverview> {
  return request('clear-lyrics-cache', () => invoke<DataOverview>('clear_lyrics_cache'))
}

/** 只清理当前歌曲的规范化歌词缓存。 */
export function clearCurrentLyricsCache(): Promise<DataOverview> {
  return request('clear-current-lyrics-cache', () =>
    invoke<DataOverview>('clear_current_lyrics_cache'),
  )
}

/** 清理当前条目并强制重新执行歌词获取链路。 */
export function refreshCurrentLyrics(): Promise<DataOverview> {
  return request('refresh-current-lyrics', () => invoke<DataOverview>('refresh_current_lyrics'))
}

/** 恢复默认配置；后端保存完成后会请求重启应用。 */
export function resetConfiguration(): Promise<void> {
  return request('reset-configuration', () => invoke('reset_configuration'))
}

/** 清空轮转历史日志并返回最新占用，活动日志继续写入。 */
export function clearLogHistory(): Promise<DataOverview> {
  return request('clear-log-history', () => invoke<DataOverview>('clear_log_history'))
}
