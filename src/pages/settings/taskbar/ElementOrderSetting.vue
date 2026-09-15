<script setup lang="ts">
import { Disc3, GripVertical, LayoutPanelLeft, ListMusic, Radio } from '@lucide/vue'
import { moveArrayElement, useSortable } from '@vueuse/integrations/useSortable'
import type { SortableEvent } from 'sortablejs'
import { nextTick, onMounted, shallowRef, useTemplateRef } from 'vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_ELEMENT_ORDER,
  getTaskbarElementOrder,
  setTaskbarElementOrder,
  type TaskbarElement,
} from '@/features/settings/element-order'

const elementOptions = {
  cover: { label: '封面', icon: Disc3, widthClass: 'w-28 flex-none' },
  'track-info': { label: '歌曲信息', icon: ListMusic, widthClass: 'min-w-32 flex-1' },
  controls: { label: '控制按钮组', icon: Radio, widthClass: 'w-40 flex-none' },
} as const

const selectedOrder = shallowRef<TaskbarElement[]>([...DEFAULT_TASKBAR_ELEMENT_ORDER])
const committedOrder = shallowRef<TaskbarElement[]>([...DEFAULT_TASKBAR_ELEMENT_ORDER])
const orderSaving = shallowRef(false)
const sortableContainer = useTemplateRef<HTMLElement>('sortableContainer')

const { option } = useSortable(sortableContainer, selectedOrder, {
  animation: 160,
  direction: 'horizontal',
  forceFallback: true,
  fallbackTolerance: 3,
  handle: '[data-drag-handle]',
  ghostClass: 'opacity-40',
  onUpdate: handleSortUpdate,
  watchElement: true,
})

/** 恢复已保存的区块排列。 */
async function loadElementOrder() {
  try {
    const order = await getTaskbarElementOrder()
    selectedOrder.value = order
    committedOrder.value = [...order]
  } catch (error) {
    console.error('读取任务栏区块顺序失败', error)
  }
}

/** 在拖动落点确定后更新数组，避免拖动过程中持续写入设置。 */
function handleSortUpdate(event: SortableEvent) {
  if (event.oldIndex === undefined || event.newIndex === undefined || orderSaving.value) return

  orderSaving.value = true
  option('disabled', true)
  moveArrayElement(selectedOrder, event.oldIndex, event.newIndex, event)
  void nextTick(() => saveElementOrder([...selectedOrder.value]))
}

/** 保存一次完整拖动结果；失败时恢复最后一次成功的排列。 */
async function saveElementOrder(order: TaskbarElement[]) {
  try {
    await setTaskbarElementOrder(order)
    committedOrder.value = [...order]
  } catch (error) {
    selectedOrder.value = [...committedOrder.value]
    notifySettingSaveFailed('任务栏区块顺序', error)
    try {
      await setTaskbarElementOrder(committedOrder.value)
    } catch (rollbackError) {
      console.error('恢复之前的任务栏区块顺序失败', rollbackError)
    }
  } finally {
    orderSaving.value = false
    await nextTick()
    option('disabled', false)
  }
}

onMounted(loadElementOrder)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-indigo-500">
      <LayoutPanelLeft />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>元素排列</ItemTitle>
      <ItemDescription>拖动调整封面、歌曲信息和控制按钮组的显示顺序</ItemDescription>
    </ItemContent>

    <template #content>
      <div
        ref="sortableContainer"
        class="bg-muted/50 flex gap-2 rounded-lg p-2"
        aria-label="任务栏元素排列"
      >
        <div
          v-for="element in selectedOrder"
          :key="element"
          class="bg-background flex min-w-0 items-center gap-2 rounded-md border p-3 shadow-xs"
          :class="elementOptions[element].widthClass"
        >
          <component
            :is="elementOptions[element].icon"
            class="size-4 shrink-0"
            aria-hidden="true"
          />
          <span class="truncate text-sm font-medium">{{ elementOptions[element].label }}</span>
          <button
            class="text-muted-foreground hover:text-foreground focus-visible:ring-ring ml-auto grid shrink-0 cursor-grab place-items-center rounded-sm outline-none focus-visible:ring-2 active:cursor-grabbing"
            type="button"
            data-drag-handle
            :disabled="orderSaving"
            :aria-label="`拖动${elementOptions[element].label}`"
          >
            <GripVertical class="size-4" aria-hidden="true" />
          </button>
        </div>
      </div>
    </template>
  </CollapsibleItem>
</template>
