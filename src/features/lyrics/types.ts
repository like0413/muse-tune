import type { MediaPlayer } from '@/features/media/types'

export type LyricsStatus = 'loading' | 'ready' | 'unavailable' | 'error'
export type LyricsPrecision = 'word' | 'line'
export type LyricsSourceKind = 'local' | 'online'

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

export interface LyricsCachePathState {
  player: MediaPlayer
  automaticPath: string | null
  overridePath: string | null
  effectivePath: string | null
  exists: boolean
}
