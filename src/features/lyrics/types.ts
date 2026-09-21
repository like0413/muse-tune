import type { MediaPlayer } from '@/features/media/types'

/**
 * 歌词解析结论，只有前三种会在任务栏上有对应渲染。
 *
 * `ready` 由歌词行组件渲染，但歌词层还额外要求 `timeline !== null`（无有效播放器时间线
 * 时不进入歌词模式）；`instrumental` / `no_lyrics` 渲染为结论提示；`loading` /
 * `unavailable` / `error` 没有渲染分支，界面保持普通模式而非空白歌词。
 */
export type LyricsStatus =
  | 'loading'
  | 'ready'
  | 'instrumental'
  | 'no_lyrics'
  | 'unavailable'
  | 'error'
export type LyricsPrecision = 'word' | 'line'
export type LyricsSourceKind = 'local' | 'online'
export type LyricsResolutionMethod = 'none' | 'application_cache' | 'player_local' | 'online'
export type LyricsResolutionOutcome = 'hit' | 'miss' | 'error'
/** 解析步骤的来源标识；展示文案由前端按当前语言组装。 */
export type LyricsResolutionSite =
  | 'application_cache'
  | 'local'
  | 'online'
  | 'online_fallback'
  | 'local_upgrade'
/** 前后端共同支持的在线歌词调度策略。 */
export const LYRICS_ONLINE_STRATEGIES = ['parallel', 'current_player_only'] as const
export type LyricsOnlineStrategy = (typeof LYRICS_ONLINE_STRATEGIES)[number]

export interface LyricsResolutionStep {
  site: LyricsResolutionSite
  outcome: LyricsResolutionOutcome
  detail: string | null
  /** 该步骤属于并发查询多个来源的阶段；同阶段的步骤是同时执行的。 */
  parallel: boolean
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

/** 一轮解析使用的曲目信息；用于判断媒体快照是否已经稳定。 */
export interface LyricsResolutionTrack {
  title: string
  artists: string[]
  durationMs: number | null
}

/** 一轮已结束的解析记录：尝试过哪些来源，以及最终结论。 */
export interface LyricsResolutionRecord {
  finishedAtSeconds: number | null
  durationMs: number | null
  track: LyricsResolutionTrack | null
  steps: LyricsResolutionStep[]
  status: LyricsStatus
  source: LyricsSource | null
  precision: LyricsPrecision | null
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
  /** 本轮解析使用的曲目信息；切歌瞬间可能混搭上一首的字段。 */
  resolutionTrack: LyricsResolutionTrack | null
  resolutionSteps: LyricsResolutionStep[]
  /** 上一轮已完成的解析（只保留一条）。 */
  recentResolutions: LyricsResolutionRecord[]
  cache: LyricsCacheDiagnostics
  adapters: LyricsAdapterDiagnostics[]
  watcher: {
    enqueuedBatches: number
    processedBatches: number
    coalescedBatches: number
    callbackCount: number
    pendingBatches: number
    pendingBatchesPeak: number
  }
}
