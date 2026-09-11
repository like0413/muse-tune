import type { LyricsDiagnostics } from '@/features/lyrics/types'
import type { MediaPlaybackStatus, MediaPlayer } from '@/features/media/types'
import type { TaskbarDisplay } from '@/features/settings/display'

export interface ApplicationDiagnostics {
  name: string
  version: string
  buildProfile: string
  targetArch: string
  targetOs: string
  cacheDirectory: string | null
  logDirectory: string | null
}

export interface TaskbarDiagnostics {
  detectedDisplayCount: number
  barWindowCount: number
  visibleBarWindowCount: number
  contentVisible: boolean
  displayTarget: string
  placement: string
  overlapPriority: string
  contentWidthDip: number
  displays: TaskbarDisplay[]
  windows: Array<{ label: string; visible: boolean }>
}

export interface MediaDiagnostics {
  sessionAvailable: boolean
  player: MediaPlayer | null
  playbackStatus: MediaPlaybackStatus | null
  title: string | null
  artist: string | null
  timelineAvailable: boolean
  timelineStartMs: number | null
  durationMs: number | null
  playbackRate: number | null
  canSeek: boolean
  canTogglePlayPause: boolean
  canSkipNext: boolean
  canSkipPrevious: boolean
  discoveredSessionCount: number | null
  selectionStrategy: string | null
  audioSessionBound: boolean
  audioProcessId: number | null
  volumeLevel: number | null
  muted: boolean | null
  spectrumEnabled: boolean | null
  spectrumActive: boolean | null
  runtimeError: string | null
  sessions: Array<{
    player: MediaPlayer
    playbackStatus: MediaPlaybackStatus
    title: string | null
    artist: string | null
    timelineAvailable: boolean
    selected: boolean
  }>
}

export interface StorageDiagnostics {
  settingsFile: string | null
  settingsFileExists: boolean
  settingsFileBytes: number | null
  logFileCount: number
  logTotalBytes: number
}

export interface DiagnosticIssue {
  severity: 'warning' | 'error'
  area: string
  message: string
}

export interface DiagnosticsSnapshot {
  application: ApplicationDiagnostics
  taskbar: TaskbarDiagnostics
  media: MediaDiagnostics
  lyrics: LyricsDiagnostics
  storage: StorageDiagnostics
  issues: DiagnosticIssue[]
}
