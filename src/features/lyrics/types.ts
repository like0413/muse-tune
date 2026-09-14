import type { MediaPlayer } from '@/features/media/types'

export type LyricsStatus = 'loading' | 'ready' | 'unavailable' | 'error'
export type LyricsPrecision = 'word' | 'line'
export type LyricsSourceKind = 'local' | 'online'
export type LyricsResolutionMethod = 'none' | 'application_cache' | 'player_local' | 'online'
export type LyricsResolutionOutcome = 'hit' | 'miss' | 'error'
export type LyricsOnlineStrategy = 'parallel' | 'current_player_first'

export interface LyricsResolutionStep {
  label: string
  outcome: LyricsResolutionOutcome
  detail: string | null
  parallelGroup: string | null
}

export interface LyricsCacheDiagnostics {
  schemaVersion: string
  entryCount: number
  totalBytes: number
  limitBytes: number
  currentEntryExists: boolean
  currentEntryBytes: number | null
  currentEntryAgeSeconds: number | null
  currentEntryFresh: boolean | null
  currentRefreshRemainingSeconds: number | null
}

export interface LyricsAdapterDiagnostics {
  player: MediaPlayer
  cachePath: string | null
  cachePathAvailable: boolean
  watcherActive: boolean
}

export interface LyricWord {
  startMs: number
  endMs: number
  text: string
}

export interface LyricLine {
  startMs: number
  endMs: number
  text: string
  translation: string | null
  romanization: string | null
  words: LyricWord[]
}

export interface LyricsSource {
  player: MediaPlayer
  kind: LyricsSourceKind
  songId: string | null
}

export interface LyricsSnapshot {
  trackKey: string | null
  status: LyricsStatus
  source: LyricsSource | null
  precision: LyricsPrecision | null
  lines: LyricLine[]
  errorReason: string | null
}

export interface LyricsSnapshotDiagnostics {
  status: LyricsStatus
  source: LyricsSource | null
  precision: LyricsPrecision | null
  lineCount: number
  errorReason: string | null
}

export interface LyricsDiagnostics {
  snapshot: LyricsSnapshotDiagnostics
  currentPlayer: MediaPlayer | null
  enabled: boolean
  onlineStrategy: LyricsOnlineStrategy
  resolutionMethod: LyricsResolutionMethod
  localCachePath: string | null
  localCacheAvailable: boolean
  resolverRunning: boolean
  pendingResolution: boolean
  resolutionDurationMs: number | null
  resolutionSteps: LyricsResolutionStep[]
  cache: LyricsCacheDiagnostics
  adapters: LyricsAdapterDiagnostics[]
}
