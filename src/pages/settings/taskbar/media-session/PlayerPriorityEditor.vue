<script setup lang="ts">
import { GripVertical, Music2 } from '@lucide/vue'
import { moveArrayElement, useSortable } from '@vueuse/integrations/useSortable'
import type { SortableEvent } from 'sortablejs'
import { nextTick, shallowRef, useTemplateRef, watch } from 'vue'

import { getMediaPlayerPresentation } from '@/features/media/players'
import type { MediaPlayer } from '@/features/media/types'

const props = defineProps<{
  players: readonly MediaPlayer[]
  disabled: boolean
}>()

const emit = defineEmits<{
  reorder: [players: MediaPlayer[]]
}>()

const editablePlayers = shallowRef<MediaPlayer[]>([...props.players])
const priorityContainer = useTemplateRef<HTMLElement>('priorityContainer')

const { option } = useSortable(priorityContainer, editablePlayers, {
  animation: 160,
  direction: 'vertical',
  forceFallback: true,
  fallbackTolerance: 3,
  handle: '[data-drag-handle]',
  ghostClass: 'opacity-40',
  onUpdate: handlePriorityUpdate,
  watchElement: true,
})

/** 父级保存成功或失败回滚后，同步权威播放器顺序。 */
watch(
  () => props.players,
  (players) => {
    editablePlayers.value = [...players]
  },
)

/** 保存期间同时禁用 Sortable 实例，避免只依赖按钮状态。 */
watch(
  () => props.disabled,
  (disabled) => option('disabled', disabled),
  { immediate: true },
)

/** 将拖拽落点转换为新的不可变播放器顺序。 */
function handlePriorityUpdate(event: SortableEvent) {
  if (event.oldIndex === undefined || event.newIndex === undefined || props.disabled) return
  moveArrayElement(editablePlayers, event.oldIndex, event.newIndex, event)
  void nextTick(() => emit('reorder', [...editablePlayers.value]))
}
</script>

<template>
  <div
    ref="priorityContainer"
    class="bg-muted/50 grid w-56 gap-2 rounded-lg p-2"
    aria-label="播放器优先级"
  >
    <div
      v-for="(player, index) in editablePlayers"
      :key="player"
      class="bg-background flex items-center gap-3 rounded-md border px-3 py-2 shadow-xs"
    >
      <span class="text-muted-foreground w-4 text-center text-xs">{{ index + 1 }}</span>
      <Music2 class="size-4" aria-hidden="true" />
      <span class="text-sm font-medium">
        {{ getMediaPlayerPresentation(player).label }}
      </span>
      <button
        class="text-muted-foreground hover:text-foreground focus-visible:ring-ring ml-auto grid cursor-grab place-items-center rounded-sm outline-none focus-visible:ring-2 active:cursor-grabbing"
        type="button"
        data-drag-handle
        :disabled="disabled"
        :aria-label="`拖动${getMediaPlayerPresentation(player).label}`"
      >
        <GripVertical class="size-4" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>
