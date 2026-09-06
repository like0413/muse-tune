<script setup lang="ts">
import { Disc3, GripVertical, LayoutPanelLeft, ListMusic, Radio } from '@lucide/vue'
import { moveArrayElement, useSortable } from '@vueuse/integrations/useSortable'
import type { SortableEvent } from 'sortablejs'
import { nextTick, onMounted, shallowRef, useTemplateRef } from 'vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Separator } from '@/components/ui/separator'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  DEFAULT_TASKBAR_ELEMENT_ORDER,
  getTaskbarElementOrder,
  setTaskbarElementOrder,
  type TaskbarElement,
} from '@/features/settings/element-order'
import {
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
  getTaskbarTrackInfoAlignment,
  isTaskbarTrackInfoAlignment,
  setTaskbarTrackInfoAlignment,
  type TaskbarTrackInfoAlignment,
} from '@/features/settings/track-info'

const elementOptions = {
  cover: { label: '封面', icon: Disc3 },
  'track-info': { label: '歌曲信息', icon: ListMusic },
  controls: { label: '控制按钮组', icon: Radio },
} as const

const alignmentOptions = [
  { value: 'left', label: '左对齐' },
  { value: 'right', label: '右对齐' },
] as const

const selectedOrder = shallowRef<TaskbarElement[]>([...DEFAULT_TASKBAR_ELEMENT_ORDER])
const committedOrder = shallowRef<TaskbarElement[]>([...DEFAULT_TASKBAR_ELEMENT_ORDER])
const orderSaving = shallowRef(false)
const selectedAlignment = shallowRef<TaskbarTrackInfoAlignment>(
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
)
const committedAlignment = shallowRef<TaskbarTrackInfoAlignment>(
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
)
const alignmentSaving = shallowRef(false)
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

/** 恢复已保存的歌曲信息对齐方式。 */
async function loadTrackInfoAlignment() {
  try {
    const alignment = await getTaskbarTrackInfoAlignment()
    selectedAlignment.value = alignment
    committedAlignment.value = alignment
  } catch (error) {
    console.error('读取歌曲信息对齐方式失败', error)
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
    console.error('保存任务栏区块顺序失败', error)
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

/** 保存歌曲信息对齐方式，失败时恢复最近一次成功值。 */
async function selectTrackInfoAlignment(value: string | number) {
  if (
    alignmentSaving.value ||
    !isTaskbarTrackInfoAlignment(value) ||
    value === selectedAlignment.value
  ) {
    return
  }

  selectedAlignment.value = value
  alignmentSaving.value = true
  try {
    await setTaskbarTrackInfoAlignment(value)
    committedAlignment.value = value
  } catch (error) {
    selectedAlignment.value = committedAlignment.value
    console.error('保存歌曲信息对齐方式失败', error)
  } finally {
    alignmentSaving.value = false
  }
}

onMounted(loadElementOrder)
onMounted(loadTrackInfoAlignment)
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
        class="bg-muted/50 grid grid-cols-3 gap-2 rounded-lg p-2"
        aria-label="任务栏元素排列"
      >
        <div
          v-for="element in selectedOrder"
          :key="element"
          class="bg-background flex min-w-0 items-center gap-2 rounded-md border p-3 shadow-xs"
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

      <Separator class="my-3" />

      <div class="flex items-center justify-between gap-4">
        <div class="grid gap-0.5">
          <span class="text-sm font-medium">歌曲信息对齐方式</span>
          <span class="text-muted-foreground text-xs">调整歌名和歌手在可用区域内的对齐方向</span>
        </div>
        <Tabs :model-value="selectedAlignment" @update:model-value="selectTrackInfoAlignment">
          <TabsList aria-label="歌曲信息对齐方式">
            <TabsTrigger
              v-for="option in alignmentOptions"
              :key="option.value"
              :value="option.value"
              :disabled="alignmentSaving"
            >
              {{ option.label }}
            </TabsTrigger>
          </TabsList>
        </Tabs>
      </div>
    </template>
  </CollapsibleItem>
</template>
