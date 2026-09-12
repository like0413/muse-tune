import { invoke } from '@tauri-apps/api/core'

import type { DataDirectoryKind, DataOverview } from './types'

/** 读取应用管理的数据目录与占用摘要。 */
export function getDataOverview(): Promise<DataOverview> {
  return invoke<DataOverview>('get_data_overview')
}

/** 使用系统文件管理器打开指定应用数据目录。 */
export function openDataDirectory(kind: DataDirectoryKind): Promise<void> {
  return invoke('open_data_directory', { kind })
}

/** 清空歌词缓存并返回最新占用。 */
export function clearLyricsCache(): Promise<DataOverview> {
  return invoke<DataOverview>('clear_lyrics_cache')
}

/** 恢复默认配置；后端保存完成后会请求重启应用。 */
export function resetConfiguration(): Promise<void> {
  return invoke('reset_configuration')
}

/** 清空轮转历史日志并返回最新占用，活动日志继续写入。 */
export function clearLogHistory(): Promise<DataOverview> {
  return invoke<DataOverview>('clear_log_history')
}
