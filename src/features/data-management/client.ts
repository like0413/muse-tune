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

export function getDataOverview(): Promise<DataOverview> {
  return request('get-overview', () => invoke<DataOverview>('get_data_overview'))
}

export function openDataDirectory(kind: DataDirectoryKind): Promise<void> {
  return request('open-directory', () => invoke('open_data_directory', { kind }))
}

export function clearLyricsCache(): Promise<DataOverview> {
  return request('clear-lyrics-cache', () => invoke<DataOverview>('clear_lyrics_cache'))
}

export function clearCurrentLyricsCache(): Promise<DataOverview> {
  return request('clear-current-lyrics-cache', () =>
    invoke<DataOverview>('clear_current_lyrics_cache'),
  )
}

export function refreshCurrentLyrics(): Promise<DataOverview> {
  return request('refresh-current-lyrics', () => invoke<DataOverview>('refresh_current_lyrics'))
}

export function resetConfiguration(): Promise<void> {
  return request('reset-configuration', () => invoke('reset_configuration'))
}

export function clearLogHistory(): Promise<DataOverview> {
  return request('clear-log-history', () => invoke<DataOverview>('clear_log_history'))
}
