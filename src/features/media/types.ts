/**
 * GSMTC 会话的播放状态，与原生侧枚举一一对应。
 *
 * `playing` 是封面旋转、歌词推进等播放行为的唯一判据，`paused` 被自动隐藏策略单独识别。
 * 播放按钮在同一播放器的 `changing` 过渡期间保留上一图标，原始状态和控制能力仍实时同步。
 * 其余 `closed` / `opened` / `stopped` / `unknown` 均按“未在播放”处理。
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
  | 'spotify'
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
