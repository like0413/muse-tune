import type { MediaPlayer } from '../types'

/** 播放器来源徽标的回退外观；系统应用图标不可用时使用。 */
export interface MediaPlayerPresentation {
  player: MediaPlayer
  label: string
  fallbackText: string
  fallbackClass: string
}
