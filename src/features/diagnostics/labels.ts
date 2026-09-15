import { translateGlobal } from '@/features/i18n'
import type { LyricsPrecision, LyricsResolutionMethod, LyricsStatus } from '@/features/lyrics/types'
import type { MediaPlaybackStatus, MediaPlayer } from '@/features/media/types'

const playerNames: Record<Exclude<MediaPlayer, 'other'>, string> = {
  qq_music: 'QQ 音乐',
  netease_cloud_music: '网易云音乐',
  soda_music: '汽水音乐',
  kugou_music: '酷狗音乐',
}

const playbackStatusKeys: Record<MediaPlaybackStatus, string> = {
  closed: 'diagnostics.values.closed',
  opened: 'diagnostics.values.opened',
  changing: 'diagnostics.values.changing',
  stopped: 'diagnostics.values.stopped',
  playing: 'diagnostics.values.playing',
  paused: 'diagnostics.values.paused',
  unknown: 'diagnostics.values.unknown',
}

const lyricsStatusKeys: Record<LyricsStatus, string> = {
  loading: 'diagnostics.values.resolving',
  ready: 'diagnostics.values.ready',
  instrumental: 'diagnostics.values.instrumental',
  unavailable: 'diagnostics.values.unavailable',
  error: 'diagnostics.values.error',
}

const resolutionMethodKeys: Record<LyricsResolutionMethod, string> = {
  none: 'common.unavailable',
  application_cache: 'diagnostics.values.applicationCache',
  player_local: 'diagnostics.values.playerCache',
  online: 'diagnostics.values.onlineFetch',
}

const precisionKeys: Record<LyricsPrecision, string> = {
  word: 'diagnostics.values.wordPrecision',
  line: 'diagnostics.values.linePrecision',
}

/** 播放器品牌沿用产品原名，仅本地化兜底类别。 */
export function getPlayerLabel(player: MediaPlayer): string {
  return player === 'other'
    ? translateGlobal('diagnostics.values.otherPlayer')
    : playerNames[player]
}

export const getPlaybackStatusLabel = (status: MediaPlaybackStatus) =>
  translateGlobal(playbackStatusKeys[status])
export const getLyricsStatusLabel = (status: LyricsStatus) =>
  translateGlobal(lyricsStatusKeys[status])
export const getResolutionMethodLabel = (method: LyricsResolutionMethod) =>
  translateGlobal(resolutionMethodKeys[method])
export const getPrecisionLabel = (precision: LyricsPrecision) =>
  translateGlobal(precisionKeys[precision])

export function formatDuration(milliseconds: number | null): string {
  if (milliseconds === null || milliseconds < 0) return translateGlobal('common.unavailable')
  const totalSeconds = Math.round(milliseconds / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}

export function formatBytes(bytes: number | null): string {
  if (bytes === null) return translateGlobal('common.unavailable')
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(bytes === 0 ? 0 : 1)} kb`
  return `${(bytes / 1024 ** 2).toFixed(1)} mb`
}

export function formatAgeSeconds(seconds: number | null): string {
  if (seconds === null) return translateGlobal('common.unavailable')
  if (seconds < 60) return translateGlobal('diagnostics.duration.seconds', { count: seconds })
  if (seconds < 3_600)
    return translateGlobal('diagnostics.duration.minutes', { count: Math.floor(seconds / 60) })
  if (seconds < 86_400)
    return translateGlobal('diagnostics.duration.hours', { count: Math.floor(seconds / 3_600) })
  return translateGlobal('diagnostics.duration.days', { count: Math.floor(seconds / 86_400) })
}
