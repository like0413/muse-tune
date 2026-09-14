import type { LyricsPrecision, LyricsResolutionMethod, LyricsStatus } from '@/features/lyrics/types'
import type { MediaPlaybackStatus, MediaPlayer } from '@/features/media/types'

export const playerLabels: Record<MediaPlayer, string> = {
  qq_music: 'QQ 音乐',
  netease_cloud_music: '网易云音乐',
  soda_music: '汽水音乐',
  kugou_music: '酷狗音乐',
  other: '其他播放器',
}

export const playbackStatusLabels: Record<MediaPlaybackStatus, string> = {
  closed: '已关闭',
  opened: '已打开',
  changing: '切换中',
  stopped: '已停止',
  playing: '播放中',
  paused: '已暂停',
  unknown: '未知',
}

export const lyricsStatusLabels: Record<LyricsStatus, string> = {
  loading: '解析中',
  ready: '已就绪',
  instrumental: '纯音乐',
  unavailable: '不可用',
  error: '错误',
}

export const resolutionMethodLabels: Record<LyricsResolutionMethod, string> = {
  none: '暂无',
  application_cache: 'Muse Tune 缓存命中',
  player_local: '播放器本地缓存',
  online: '在线重新获取',
}

export const precisionLabels: Record<LyricsPrecision, string> = {
  word: '逐字',
  line: '逐行',
}

export function formatDuration(milliseconds: number | null): string {
  if (milliseconds === null || milliseconds < 0) return '暂无'
  const totalSeconds = Math.round(milliseconds / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}

export function formatBytes(bytes: number | null): string {
  if (bytes === null) return '暂无'
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(bytes === 0 ? 0 : 1)} kb`
  return `${(bytes / 1024 ** 2).toFixed(1)} mb`
}

export function formatAgeSeconds(seconds: number | null): string {
  if (seconds === null) return '暂无'
  if (seconds < 60) return `${seconds} 秒`
  if (seconds < 3_600) return `${Math.floor(seconds / 60)} 分钟`
  if (seconds < 86_400) return `${Math.floor(seconds / 3_600)} 小时`
  return `${Math.floor(seconds / 86_400)} 天`
}
