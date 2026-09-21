<script setup lang="ts">
import { GripVertical } from '@lucide/vue'
import { moveArrayElement, useSortable } from '@vueuse/integrations/useSortable'
import type { SortableEvent } from 'sortablejs'

import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import { getMediaPlayerLabel } from '@/features/media/players'
import type { MediaPlayer } from '@/features/media/types'

const { t } = useI18n({ useScope: 'global' })

const props = defineProps<{
  order: readonly MediaPlayer[]
  enabled: readonly MediaPlayer[]
  disabled: boolean
}>()

const emit = defineEmits<{
  reorder: [order: MediaPlayer[]]
  toggle: [player: MediaPlayer, enabled: boolean]
}>()

const editableOrder = shallowRef<MediaPlayer[]>([...props.order])
const orderContainer = useTemplateRef<HTMLElement>('orderContainer')

const { option } = useSortable(orderContainer, editableOrder, {
  animation: 160,
  direction: 'vertical',
  forceFallback: true,
  fallbackTolerance: 3,
  handle: '[data-drag-handle]',
  ghostClass: 'opacity-40',
  onUpdate: handleOrderUpdate,
  watchElement: true,
})

/** 父级保存成功或失败回滚后，同步权威在线接口顺序。 */
watch(
  () => props.order,
  (order) => {
    editableOrder.value = [...order]
  },
)

/** 保存期间同时禁用 Sortable 实例，避免只依赖按钮状态。 */
watch(
  () => props.disabled,
  (disabled) => option('disabled', disabled),
  { immediate: true },
)

/** 将拖拽落点转换为新的不可变接口顺序。 */
function handleOrderUpdate(event: SortableEvent) {
  if (event.oldIndex === undefined || event.newIndex === undefined || props.disabled) return
  moveArrayElement(editableOrder, event.oldIndex, event.newIndex, event)
  void nextTick(() => emit('reorder', [...editableOrder.value]))
}

/** 将 Checkbox 值规范为布尔值并上报勾选结果。 */
function toggleSource(player: MediaPlayer, value: boolean | 'indeterminate') {
  emit('toggle', player, value === true)
}
</script>

<template>
  <div ref="orderContainer" class="bg-muted/50 grid w-56 gap-2 rounded-lg p-2">
    <div
      v-for="player in editableOrder"
      :key="player"
      class="bg-background flex items-center gap-2 rounded-md border px-3 py-2 shadow-xs"
    >
      <Checkbox
        :id="`lyrics-online-source-${player}`"
        :model-value="enabled.includes(player)"
        :disabled="disabled"
        @update:model-value="toggleSource(player, $event)"
      />
      <Label :for="`lyrics-online-source-${player}`" class="min-w-0 flex-1">
        <span class="block truncate text-sm">{{ getMediaPlayerLabel(player) }}</span>
      </Label>
      <button
        class="text-muted-foreground hover:text-foreground focus-visible:ring-ring ml-auto grid cursor-grab place-items-center rounded-sm outline-none focus-visible:ring-2 active:cursor-grabbing"
        type="button"
        data-drag-handle
        :disabled="disabled"
        :aria-label="t('common.dragItem', { item: getMediaPlayerLabel(player) })"
      >
        <GripVertical class="size-4" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>
