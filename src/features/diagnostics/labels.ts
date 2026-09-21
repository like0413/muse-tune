import { translateGlobal } from '@/features/i18n'
import type {
  LyricsPrecision,
  LyricsResolutionMethod,
  LyricsResolutionSite,
  LyricsStatus,
} from '@/features/lyrics/types'
import type { MediaPlaybackStatus } from '@/features/media/types'

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
  no_lyrics: 'diagnostics.values.noLyrics',
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

const resolutionSiteKeys: Record<LyricsResolutionSite, string> = {
  application_cache: 'diagnostics.lyrics.sites.applicationCache',
  local: 'diagnostics.lyrics.sites.local',
  online: 'diagnostics.lyrics.sites.online',
  online_fallback: 'diagnostics.lyrics.sites.onlineFallback',
  local_upgrade: 'diagnostics.lyrics.sites.localUpgrade',
}

export const getPlaybackStatusLabel = (status: MediaPlaybackStatus) =>
  translateGlobal(playbackStatusKeys[status])
export const getLyricsStatusLabel = (status: LyricsStatus) =>
  translateGlobal(lyricsStatusKeys[status])
export const getResolutionMethodLabel = (method: LyricsResolutionMethod) =>
  translateGlobal(resolutionMethodKeys[method])
export const getPrecisionLabel = (precision: LyricsPrecision) =>
  translateGlobal(precisionKeys[precision])
export const getResolutionSiteLabel = (site: LyricsResolutionSite) =>
  translateGlobal(resolutionSiteKeys[site])

export function formatDuration(milliseconds: number | null): string {
  if (milliseconds === null || milliseconds < 0) return translateGlobal('common.unavailable')
  const totalSeconds = Math.round(milliseconds / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
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
