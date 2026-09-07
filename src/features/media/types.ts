export type MediaPlaybackStatus =
  | 'closed'
  | 'opened'
  | 'changing'
  | 'stopped'
  | 'playing'
  | 'paused'
  | 'unknown'

export type MediaControlAction = 'toggle_play_pause' | 'skip_next' | 'skip_previous'
export type MediaPlayer =
  | 'qq_music'
  | 'netease_cloud_music'
  | 'soda_music'
  | 'kugou_music'
  | 'other'
export type MediaSessionSelectionStrategy =
  | 'follow_windows'
  | 'recent_playback'
  | 'sticky_current'
  | 'fixed_priority'

export interface MediaSessionSelectionPolicy {
  strategy: MediaSessionSelectionStrategy
  playerPriority: MediaPlayer[]
}

export interface MediaMetadata {
  title: string
  artist: string
  albumArtist: string
  subtitle: string
  thumbnailDataUrl: string | null
}

export interface MediaPlaybackControls {
  canPlay: boolean
  canPause: boolean
  canTogglePlayPause: boolean
  canSkipNext: boolean
  canSkipPrevious: boolean
}

export interface MediaPlayback {
  status: MediaPlaybackStatus
  controls: MediaPlaybackControls
}

export interface MediaSessionSnapshot {
  sourceIconDataUrl: string | null
  player: MediaPlayer
  metadata: MediaMetadata
  playback: MediaPlayback
}
