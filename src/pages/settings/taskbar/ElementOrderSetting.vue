<script setup lang="ts">
import { Disc3, GripVertical, LayoutPanelLeft, ListMusic, Radio, Text } from '@lucide/vue'
import { moveArrayElement, useSortable } from '@vueuse/integrations/useSortable'
import type { SortableEvent } from 'sortablejs'
import type { ShallowRef } from 'vue'

import { Field, FieldContent, FieldGroup, FieldTitle } from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { notifySettingSaveFailed, reportBackgroundFailure } from '@/features/feedback/errors'
import {
  cloneTaskbarElementOrder,
  DEFAULT_TASKBAR_ELEMENT_ORDER,
  getTaskbarElementOrder,
  setTaskbarElementOrder,
  type TaskbarElement,
  type TaskbarElementOrder,
  type TaskbarLyricsElement,
} from '@/features/settings/element-order'

const { t } = useI18n({ useScope: 'global' })

const elementOptions = computed(() => ({
  cover: { label: t('settings.taskbar.cover.title'), icon: Disc3, widthClass: 'w-28 flex-none' },
  'track-info': {
    label: t('settings.taskbar.trackInfo.title'),
    icon: ListMusic,
    widthClass: 'min-w-32 flex-1',
  },
  controls: {
    label: t('settings.taskbar.controls.title'),
    icon: Radio,
    widthClass: 'w-40 flex-none',
  },
  lyrics: {
    label: t('settings.taskbar.lyrics.title'),
    icon: Text,
    widthClass: 'min-w-32 flex-1',
  },
}))

const selectedNormalOrder = shallowRef<TaskbarElement[]>([...DEFAULT_TASKBAR_ELEMENT_ORDER.normal])
const selectedLyricsOrder = shallowRef<TaskbarLyricsElement[]>([
  ...DEFAULT_TASKBAR_ELEMENT_ORDER.lyrics,
])
const committedOrder = shallowRef<TaskbarElementOrder>(
  cloneTaskbarElementOrder(DEFAULT_TASKBAR_ELEMENT_ORDER),
)
const orderSaving = shallowRef(false)
const normalOrderContainer = useTemplateRef<HTMLElement>('normalOrderContainer')
const lyricsOrderContainer = useTemplateRef<HTMLElement>('lyricsOrderContainer')

const normalSortable = useSortable(normalOrderContainer, selectedNormalOrder, {
  animation: 160,
  direction: 'horizontal',
  forceFallback: true,
  fallbackTolerance: 3,
  handle: '[data-drag-handle]',
  ghostClass: 'opacity-40',
  onUpdate: (event) => handleSortUpdate(selectedNormalOrder, event),
  watchElement: true,
})
const lyricsSortable = useSortable(lyricsOrderContainer, selectedLyricsOrder, {
  animation: 160,
  direction: 'horizontal',
  forceFallback: true,
  fallbackTolerance: 3,
  handle: '[data-drag-handle]',
  ghostClass: 'opacity-40',
  onUpdate: (event) => handleSortUpdate(selectedLyricsOrder, event),
  watchElement: true,
})

/** 同时启用或禁用两组拖动，避免保存期间再次改变任一顺序。 */
function setSortingDisabled(disabled: boolean) {
  normalSortable.option('disabled', disabled)
  lyricsSortable.option('disabled', disabled)
}

/** 恢复已保存的区块排列。 */
async function loadElementOrder() {
  try {
    const order = await getTaskbarElementOrder()
    selectedNormalOrder.value = [...order.normal]
    selectedLyricsOrder.value = [...order.lyrics]
    committedOrder.value = cloneTaskbarElementOrder(order)
  } catch (error) {
    reportBackgroundFailure('读取任务栏区块顺序失败', error)
  }
}

/** 在拖动落点确定后更新数组，避免拖动过程中持续写入设置。 */
function handleSortUpdate<T>(order: ShallowRef<T[]>, event: SortableEvent) {
  if (event.oldIndex === undefined || event.newIndex === undefined || orderSaving.value) return

  orderSaving.value = true
  setSortingDisabled(true)
  moveArrayElement(order, event.oldIndex, event.newIndex, event)
  void nextTick(saveElementOrder)
}

/** 保存两种模式的完整顺序；失败时同时恢复最近一次成功值。 */
async function saveElementOrder() {
  const order: TaskbarElementOrder = {
    normal: [...selectedNormalOrder.value],
    lyrics: [...selectedLyricsOrder.value],
  }
  try {
    await setTaskbarElementOrder(order)
    committedOrder.value = cloneTaskbarElementOrder(order)
  } catch (error) {
    selectedNormalOrder.value = [...committedOrder.value.normal]
    selectedLyricsOrder.value = [...committedOrder.value.lyrics]
    notifySettingSaveFailed(t('settings.taskbar.elementOrder.title'), error)
    try {
      await setTaskbarElementOrder(committedOrder.value)
    } catch (rollbackError) {
      reportBackgroundFailure('恢复之前的任务栏区块顺序失败', rollbackError)
    }
  } finally {
    orderSaving.value = false
    await nextTick()
    setSortingDisabled(false)
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
      <ItemTitle>{{ t('settings.taskbar.elementOrder.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.elementOrder.description') }}</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup class="gap-2">
        <Field>
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.elementOrder.normal') }}</FieldTitle>
          </FieldContent>
          <div ref="normalOrderContainer" class="bg-muted/50 flex gap-2 rounded-lg p-2">
            <div
              v-for="element in selectedNormalOrder"
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
                :aria-label="t('common.dragItem', { item: elementOptions[element].label })"
              >
                <GripVertical class="size-4" aria-hidden="true" />
              </button>
            </div>
          </div>
        </Field>

        <Field>
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.elementOrder.lyrics') }}</FieldTitle>
          </FieldContent>
          <div ref="lyricsOrderContainer" class="bg-muted/50 flex gap-2 rounded-lg p-2">
            <div
              v-for="element in selectedLyricsOrder"
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
                :aria-label="t('common.dragItem', { item: elementOptions[element].label })"
              >
                <GripVertical class="size-4" aria-hidden="true" />
              </button>
            </div>
          </div>
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
