<script setup lang="ts">
import { Pause, Play, SkipBack, SkipForward } from '@lucide/vue'
import type { UnlistenFn } from '@tauri-apps/api/event'

import { Button } from '@/components/ui/button'
import type { MediaControlAction, MediaSessionSnapshot } from '@/features/media/types'
import {
  DEFAULT_TASKBAR_CONTROLS_VISIBILITY,
  getTaskbarControlsVisibility,
  listenTaskbarControlsVisibilityChange,
  type TaskbarControlButton,
  type TaskbarControlsVisibility,
} from '@/features/settings/controls'

import VolumeControlElement from './VolumeControlElement.vue'

const { t } = useI18n({ useScope: 'global' })

const props = defineProps<{
  session: MediaSessionSnapshot | null
  pending: boolean
  themeColor: string
  compact: boolean
}>()
const emit = defineEmits<{ control: [action: MediaControlAction] }>()
const visibility = shallowRef<TaskbarControlsVisibility>({ ...DEFAULT_TASKBAR_CONTROLS_VISIBILITY })
let unlistenVisibilityChange: UnlistenFn | undefined

const isPlaying = computed(() => props.session?.playback.status === 'playing')
const controlSize = computed(() => (props.compact ? 'icon-xs' : 'icon-sm'))
const canTogglePlayback = computed(() => {
  const playback = props.session?.playback
  if (!playback) return false
  return (
    playback.controls.canTogglePlayPause ||
    (isPlaying.value ? playback.controls.canPause : playback.controls.canPlay)
  )
})

type StandardControlButton = Exclude<TaskbarControlButton, 'volume'>

interface StandardControlItem {
  key: StandardControlButton
  kind: 'button'
  label: string
  icon: typeof Play
  action: MediaControlAction
  disabled: boolean
}

interface VolumeControlItem {
  key: 'volume'
  kind: 'volume'
}

type ControlItem = StandardControlItem | VolumeControlItem

/** 按已保存顺序生成当前可见按钮，并集中派生禁用状态与图标。 */
const controlItems = computed<ControlItem[]>(() => {
  const items: ControlItem[] = []
  for (const button of visibility.value.order) {
    if (!visibility.value[button]) continue
    switch (button) {
      case 'previous':
        items.push({
          key: button,
          kind: 'button',
          label: t('media.previous'),
          icon: SkipBack,
          action: 'skip_previous',
          disabled: props.pending || !props.session?.playback.controls.canSkipPrevious,
        })
        break
      case 'playPause':
        items.push({
          key: button,
          kind: 'button',
          label: t('media.playPause'),
          icon: isPlaying.value ? Pause : Play,
          action: 'toggle_play_pause',
          disabled: props.pending || !canTogglePlayback.value,
        })
        break
      case 'next':
        items.push({
          key: button,
          kind: 'button',
          label: t('media.next'),
          icon: SkipForward,
          action: 'skip_next',
          disabled: props.pending || !props.session?.playback.controls.canSkipNext,
        })
        break
      case 'volume':
        items.push({ key: button, kind: 'volume' })
        break
    }
  }
  return items
})

/** 恢复按钮显示配置，并接收设置窗口的实时更新。 */
async function initializeVisibility() {
  try {
    unlistenVisibilityChange = await listenTaskbarControlsVisibilityChange((value) => {
      visibility.value = value
    })
    visibility.value = await getTaskbarControlsVisibility()
  } catch (error) {
    console.error('初始化控制按钮配置失败', error)
  }
}

onMounted(initializeVisibility)
onUnmounted(() => unlistenVisibilityChange?.())
</script>

<template>
  <div v-if="visibility.visible" class="flex shrink-0">
    <template v-for="item in controlItems" :key="item.key">
      <VolumeControlElement
        v-if="item.kind === 'volume'"
        :theme-color="themeColor"
        :compact="compact"
      />
      <Button
        v-else
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
    </template>
  </div>
</template>

<style scoped>
/* Hover 背景以当前实际前景色生成，透明 bar 也能保持对比度。 */
.taskbar-control:hover {
  color: inherit;
  background-color: color-mix(in srgb, currentColor 20%, transparent);
}
</style>
