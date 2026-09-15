<script setup lang="ts">
import { Gamepad2, GripVertical } from '@lucide/vue'
import { moveArrayElement, useSortable } from '@vueuse/integrations/useSortable'
import type { SortableEvent } from 'sortablejs'
import { nextTick, onMounted, shallowRef, useTemplateRef } from 'vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldTitle,
} from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_CONTROLS_VISIBILITY,
  getTaskbarControlsVisibility,
  setTaskbarControlsVisibility,
  type TaskbarControlButton,
  type TaskbarControlsVisibility,
} from '@/features/settings/controls'

const { t } = useI18n({ useScope: 'global' })

const buttonOptions = computed<Record<TaskbarControlButton, { label: string }>>(() => ({
  previous: { label: t('media.previous') },
  playPause: { label: t('media.playPause') },
  next: { label: t('media.next') },
  volume: { label: t('media.volume') },
}))

const selectedVisibility = shallowRef<TaskbarControlsVisibility>({
  ...DEFAULT_TASKBAR_CONTROLS_VISIBILITY,
})
const committedVisibility = shallowRef<TaskbarControlsVisibility>({
  ...DEFAULT_TASKBAR_CONTROLS_VISIBILITY,
})
const visibilitySaving = shallowRef(false)
const selectedOrder = shallowRef<TaskbarControlButton[]>([
  ...DEFAULT_TASKBAR_CONTROLS_VISIBILITY.order,
])
const orderContainer = useTemplateRef<HTMLElement>('orderContainer')

const { option } = useSortable(orderContainer, selectedOrder, {
  animation: 160,
  direction: 'horizontal',
  forceFallback: true,
  fallbackTolerance: 3,
  handle: '[data-drag-handle]',
  ghostClass: 'opacity-40',
  onUpdate: handleSortUpdate,
  watchElement: true,
})

/** 恢复已保存的控制按钮配置。 */
async function loadVisibility() {
  try {
    const visibility = await getTaskbarControlsVisibility()
    selectedVisibility.value = visibility
    selectedOrder.value = [...visibility.order]
    committedVisibility.value = { ...visibility }
  } catch (error) {
    console.error('读取控制按钮配置失败', error)
  }
}

/** 合并并持久化一次按钮配置变更，失败时恢复最近成功值。 */
async function updateVisibility(patch: Partial<TaskbarControlsVisibility>) {
  if (visibilitySaving.value) return

  const nextVisibility = { ...selectedVisibility.value, ...patch }
  selectedVisibility.value = nextVisibility
  visibilitySaving.value = true
  try {
    await setTaskbarControlsVisibility(nextVisibility)
    committedVisibility.value = { ...nextVisibility }
  } catch (error) {
    selectedVisibility.value = { ...committedVisibility.value }
    notifySettingSaveFailed(t('settings.taskbar.controls.title'), error)
  } finally {
    visibilitySaving.value = false
  }
}

/** 将 Checkbox 值规范为布尔值并更新单个按钮。 */
function updateButton(key: TaskbarControlButton, value: boolean | 'indeterminate') {
  void updateVisibility({ [key]: value === true })
}

/** 拖动结束后一次性保存完整按钮顺序。 */
function handleSortUpdate(event: SortableEvent) {
  if (event.oldIndex === undefined || event.newIndex === undefined || visibilitySaving.value) return

  visibilitySaving.value = true
  option('disabled', true)
  moveArrayElement(selectedOrder, event.oldIndex, event.newIndex, event)
  void nextTick(() => saveOrder([...selectedOrder.value]))
}

/** 保存按钮顺序，失败时恢复最后一次成功的排列。 */
async function saveOrder(order: TaskbarControlButton[]) {
  const next = { ...selectedVisibility.value, order }
  selectedVisibility.value = next
  try {
    await setTaskbarControlsVisibility(next)
    committedVisibility.value = { ...next, order: [...order] }
  } catch (error) {
    const restoredOrder = [...committedVisibility.value.order]
    selectedOrder.value = restoredOrder
    selectedVisibility.value = { ...committedVisibility.value, order: restoredOrder }
    notifySettingSaveFailed(t('settings.taskbar.controls.order'), error)
  } finally {
    visibilitySaving.value = false
    await nextTick()
    option('disabled', false)
  }
}

onMounted(loadVisibility)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-violet-500">
      <Gamepad2 />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.controls.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.controls.description') }}</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-controls-visible">{{
              t('settings.taskbar.controls.visible')
            }}</FieldLabel>
            <FieldDescription>{{
              t('settings.taskbar.controls.visibleDescription')
            }}</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-controls-visible"
            :model-value="selectedVisibility.visible"
            :disabled="visibilitySaving"
            @update:model-value="updateVisibility({ visible: $event })"
          />
        </Field>

        <Field :data-disabled="!selectedVisibility.visible">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.controls.buttons') }}</FieldTitle>
          </FieldContent>
          <div ref="orderContainer" class="grid grid-cols-4 gap-2">
            <div
              v-for="button in selectedOrder"
              :key="button"
              class="bg-muted/50 flex min-w-0 items-center gap-2 rounded-md border p-3"
            >
              <Checkbox
                :id="`taskbar-control-${button}`"
                :model-value="selectedVisibility[button]"
                :disabled="visibilitySaving || !selectedVisibility.visible"
                @update:model-value="updateButton(button, $event)"
              />
              <Label :for="`taskbar-control-${button}`" class="min-w-0 flex-1">
                <span class="block truncate text-sm">{{ buttonOptions[button].label }}</span>
              </Label>
              <button
                class="text-muted-foreground hover:text-foreground focus-visible:ring-ring grid shrink-0 cursor-grab place-items-center rounded-sm outline-none focus-visible:ring-2 active:cursor-grabbing"
                type="button"
                data-drag-handle
                :disabled="visibilitySaving || !selectedVisibility.visible"
                :aria-label="t('common.dragItem', { item: buttonOptions[button].label })"
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
