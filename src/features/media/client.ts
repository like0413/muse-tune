import { invoke } from '@tauri-apps/api/core'

import type {
  MediaControlAction,
  MediaSessionSelectionPolicy,
  MediaSessionSnapshot,
  MediaVolumeSnapshot,
} from './types'

export const MEDIA_SESSION_CHANGED_EVENT = 'media://session-changed'
export const MEDIA_TIMELINE_CHANGED_EVENT = 'media://timeline-changed'
export const MEDIA_VOLUME_CHANGED_EVENT = 'media://volume-changed'
export const MEDIA_SPECTRUM_CHANGED_EVENT = 'media://spectrum-changed'

export function getCurrentMediaSession(): Promise<MediaSessionSnapshot | null> {
  return invoke<MediaSessionSnapshot | null>('get_current_media_session')
}

/**
 * 向当前媒体会话发送控制动作。
 *
 * 返回值表示播放器是否接受该动作；被拒绝时调用方不得自行改动本地播放状态，
 * 只能等系统事件回灌真实结果（见 `useMediaSession`）。
 */
export function controlMediaSession(action: MediaControlAction): Promise<boolean> {
  return invoke<boolean>('control_media_session', { action })
}

export function toggleCurrentMediaPlayer(): Promise<void> {
  return invoke('toggle_current_media_player')
}

export function getCurrentMediaVolume(): Promise<MediaVolumeSnapshot | null> {
  return invoke<MediaVolumeSnapshot | null>('get_current_media_volume')
}

export function setCurrentMediaVolume(level: number): Promise<MediaVolumeSnapshot> {
  return invoke<MediaVolumeSnapshot>('set_current_media_volume', { level })
}

export function toggleCurrentMediaMute(): Promise<MediaVolumeSnapshot> {
  return invoke<MediaVolumeSnapshot>('toggle_current_media_mute')
}

export function setMediaSpectrumEnabled(enabled: boolean, frameRate: number): Promise<void> {
  return invoke('set_media_spectrum_enabled', { enabled, frameRate })
}

export function setMediaSessionSelectionPolicy(policy: MediaSessionSelectionPolicy): Promise<void> {
  return invoke('set_media_session_selection_policy', { policy })
}
