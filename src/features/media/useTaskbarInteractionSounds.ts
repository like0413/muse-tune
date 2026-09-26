import playerToggleSoundUrl from '@/assets/sounds/player-toggle.wav?url'
import volumeStepSoundUrl from '@/assets/sounds/volume-step.wav?url'
import { useInteractionSounds } from '@/features/audio/useInteractionSounds'

const taskbarSoundSources = {
  playerToggle: playerToggleSoundUrl,
  volumeStep: volumeStepSoundUrl,
} as const

/** 为任务栏交互提供语义明确的音效入口。 */
export function useTaskbarInteractionSounds() {
  const { playInteractionSound } = useInteractionSounds(taskbarSoundSources)

  /** 播放一次播放器窗口切换确认音。 */
  function playPlayerToggleSound() {
    playInteractionSound('playerToggle')
  }

  /** 播放一次音量步进确认音。 */
  function playVolumeStepSound() {
    playInteractionSound('volumeStep')
  }

  return { playPlayerToggleSound, playVolumeStepSound }
}
