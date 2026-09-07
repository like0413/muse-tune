import type { MediaPlayer } from '../types'
import { kugouMusicPresentation } from './kugou-music'
import { neteaseCloudMusicPresentation } from './netease-cloud-music'
import { otherPlayerPresentation } from './other'
import { qqMusicPresentation } from './qq-music'
import { sodaMusicPresentation } from './soda-music'
import type { MediaPlayerPresentation } from './types'

const playerPresentations: Record<MediaPlayer, MediaPlayerPresentation> = {
  qq_music: qqMusicPresentation,
  netease_cloud_music: neteaseCloudMusicPresentation,
  soda_music: sodaMusicPresentation,
  kugou_music: kugouMusicPresentation,
  other: otherPlayerPresentation,
}

/** 获取播放器隔离定义的展示信息。 */
export function getMediaPlayerPresentation(player: MediaPlayer): MediaPlayerPresentation {
  return playerPresentations[player]
}
