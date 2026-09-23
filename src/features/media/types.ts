/**
 * GSMTC 会话的播放状态，与原生侧枚举一一对应。
 *
 * 前端只区分两种：`playing` 是唯一的“正在播放”判据（播放/暂停图标、封面旋转、歌词推进
 * 都只认它），`paused` 只被任务栏自动隐藏策略单独识别。其余 `closed` / `opened` /
 * `changing` / `stopped` / `unknown` 一律按“未在播放”处理，不区分也不显示。
 */
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
  onlySupportedPlayers: boolean
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

export interface MediaTimeline {
  startTimeMs: number
  endTimeMs: number
  positionMs: number
  minSeekTimeMs: number
  maxSeekTimeMs: number
  playbackRate: number
  canSeek: boolean
}

export interface MediaSessionSnapshot {
  sourceIconDataUrl: string | null
  player: MediaPlayer
  metadata: MediaMetadata
  playback: MediaPlayback
  timeline: MediaTimeline | null
}

export interface MediaVolumeSnapshot {
  /** Core Audio 标量音量，原生侧钳制在 0–1；与静音相互独立，静音时该值不会归零。 */
  level: number
  muted: boolean
}
