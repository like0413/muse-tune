export type DataDirectoryKind = 'cache' | 'config' | 'logs'

export interface CacheOverview {
  entryCount: number
  usedBytes: number
  capacityBytes: number
}

export interface ConfigOverview {
  settingsFileExists: boolean
  settingsFileBytes: number | null
}

export interface LogsOverview {
  fileCount: number
  totalBytes: number
}

export interface DataOverview {
  cache: CacheOverview
  config: ConfigOverview
  logs: LogsOverview
}
