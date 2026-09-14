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

/** 读取当前媒体会话快照。 */
export function getCurrentMediaSession(): Promise<MediaSessionSnapshot | null> {
  return invoke<MediaSessionSnapshot | null>('get_current_media_session')
}

/** 向当前媒体会话发送控制动作。 */
export function controlMediaSession(action: MediaControlAction): Promise<boolean> {
  return invoke<boolean>('control_media_session', { action })
}

/** 读取当前应用音量。 */
export function getCurrentMediaVolume(): Promise<MediaVolumeSnapshot | null> {
  return invoke<MediaVolumeSnapshot | null>('get_current_media_volume')
}

/** 设置当前应用音量。 */
export function setCurrentMediaVolume(level: number): Promise<MediaVolumeSnapshot> {
  return invoke<MediaVolumeSnapshot>('set_current_media_volume', { level })
}

/** 切换当前应用静音状态。 */
export function toggleCurrentMediaMute(): Promise<MediaVolumeSnapshot> {
  return invoke<MediaVolumeSnapshot>('toggle_current_media_mute')
}

/** 设置原生频谱采集开关。 */
export function setMediaSpectrumEnabled(enabled: boolean): Promise<void> {
  return invoke('set_media_spectrum_enabled', { enabled })
}

/** 设置媒体会话选择策略。 */
export function setMediaSessionSelectionPolicy(policy: MediaSessionSelectionPolicy): Promise<void> {
  return invoke('set_media_session_selection_policy', { policy })
}
