<script setup lang="ts">
import { Pause, Play, SkipBack, SkipForward } from '@lucide/vue'

import { Button } from '@/components/ui/button'
import { useEventState } from '@/features/ipc/useEventState'
import type { MediaControlAction, MediaSessionSnapshot } from '@/features/media/types'
import {
  DEFAULT_TASKBAR_CONTROLS_VISIBILITY,
  getTaskbarControlsVisibility,
  listenTaskbarControlsVisibilityChange,
} from '@/features/settings/controls'

const { t } = useI18n({ useScope: 'global' })

const props = defineProps<{
  session: MediaSessionSnapshot | null
  pending: boolean
  compact: boolean
}>()
const emit = defineEmits<{
  control: [action: MediaControlAction]
}>()
const visibility = useEventState(
  {
    read: getTaskbarControlsVisibility,
    subscribe: listenTaskbarControlsVisibilityChange,
    failureMessage: '初始化控制按钮配置失败',
  },
  { ...DEFAULT_TASKBAR_CONTROLS_VISIBILITY },
)

const isPlaying = shallowRef(false)
/** 切歌中的 changing 没有声明暂停，图标保持同一播放器上一状态；结束后立即同步。 */
watch(
  [() => props.session?.player ?? null, () => props.session?.playback.status ?? 'closed'],
  ([player, status], [previousPlayer]) => {
    if (player !== null && player === previousPlayer && status === 'changing') return
    isPlaying.value = status === 'playing'
  },
  { immediate: true },
)
const controlSize = computed(() => (props.compact ? 'icon-xs' : 'icon-sm'))
const canTogglePlayback = computed(() => {
  const playback = props.session?.playback
  if (!playback) return false
  return (
    playback.controls.canTogglePlayPause ||
    // 图标的过渡保持仅影响展示，控制能力仍按系统原始状态判断。
    (playback.status === 'playing' ? playback.controls.canPause : playback.controls.canPlay)
  )
})

interface ControlItem {
  key: 'previous' | 'playPause' | 'next'
  label: string
  icon: typeof Play
  action: MediaControlAction
  disabled: boolean
}

/** 按已保存顺序生成当前可见按钮，并集中派生禁用状态与图标。 */
const controlItems = computed<ControlItem[]>(() => {
  const items: ControlItem[] = []
  for (const button of visibility.value.order) {
    if (!visibility.value[button]) continue
    switch (button) {
      case 'previous':
        items.push({
          key: button,
          label: t('media.previous'),
          icon: SkipBack,
          action: 'skip_previous',
          disabled: props.pending || !props.session?.playback.controls.canSkipPrevious,
        })
        break
      case 'playPause':
        items.push({
          key: button,
          label: t('media.playPause'),
          icon: isPlaying.value ? Pause : Play,
          action: 'toggle_play_pause',
          disabled: props.pending || !canTogglePlayback.value,
        })
        break
      case 'next':
        items.push({
          key: button,
          label: t('media.next'),
          icon: SkipForward,
          action: 'skip_next',
          disabled: props.pending || !props.session?.playback.controls.canSkipNext,
        })
        break
    }
  }
  return items
})
</script>

<template>
  <div v-if="visibility.visible" class="flex shrink-0">
    <Button
      v-for="item in controlItems"
      :key="item.key"
      variant="ghost"
      :size="controlSize"
      class="taskbar-control"
      type="button"
      :aria-label="item.label"
      :disabled="item.disabled"
      @click="emit('control', item.action)"
    >
      <component :is="item.icon" class="fill-current" data-icon="inline-start" />
    </Button>
  </div>
</template>

<style scoped>
/* Hover 背景以当前实际前景色生成，透明 bar 也能保持对比度。 */
.taskbar-control:hover {
  color: inherit;
  background-color: color-mix(in srgb, currentColor 20%, transparent);
}
</style>
