<script setup lang="ts">
import { CirclePlay, GripVertical } from '@lucide/vue'
import { moveArrayElement, useSortable } from '@vueuse/integrations/useSortable'
import type { SortableEvent } from 'sortablejs'

import { Checkbox } from '@/components/ui/checkbox'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldTitle,
} from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import {
  notifyActionFailed,
  notifySettingSaveFailed,
  reportBackgroundFailure,
} from '@/features/feedback/errors'
import { setCurrentMediaVolume } from '@/features/media/client'
import {
  DEFAULT_TASKBAR_CONTROLS_VISIBILITY,
  getTaskbarControlsVisibility,
  setTaskbarControlsVisibility,
  type TaskbarControlButton,
  type TaskbarControlsVisibility,
} from '@/features/settings/controls'
import {
  DEFAULT_VOLUME_CONTROL_TARGET,
  getVolumeControlTarget,
  isVolumeControlTarget,
  setVolumeControlTarget,
  type VolumeControlTarget,
} from '@/features/settings/volume-control'

import VolumeTargetConfirmDialog from './components/VolumeTargetConfirmDialog.vue'

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
const selectedVolumeTarget = shallowRef(DEFAULT_VOLUME_CONTROL_TARGET)
const committedVolumeTarget = shallowRef(DEFAULT_VOLUME_CONTROL_TARGET)
const volumeTargetSaving = shallowRef(false)
const pendingSystemVolumeSwitch = shallowRef(false)
const volumeTargetDialogOpen = computed({
  get: () => pendingSystemVolumeSwitch.value,
  set: (open: boolean) => {
    if (!open && !volumeTargetSaving.value) pendingSystemVolumeSwitch.value = false
  },
})
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
    reportBackgroundFailure('读取控制按钮配置失败', error)
  }
}

/** 恢复已保存的音量控制对象。 */
async function loadVolumeTarget() {
  try {
    const target = await getVolumeControlTarget()
    selectedVolumeTarget.value = target
    committedVolumeTarget.value = target
  } catch (error) {
    reportBackgroundFailure('读取音量控制对象失败', error)
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

/** 保存音量控制对象，失败时恢复最近一次成功值。 */
async function saveVolumeTarget(value: VolumeControlTarget): Promise<boolean> {
  if (volumeTargetSaving.value) return false
  selectedVolumeTarget.value = value
  volumeTargetSaving.value = true
  try {
    await setVolumeControlTarget(value)
    committedVolumeTarget.value = value
    return true
  } catch (error) {
    selectedVolumeTarget.value = committedVolumeTarget.value
    notifySettingSaveFailed(t('settings.taskbar.controls.volumeTarget'), error)
    return false
  } finally {
    volumeTargetSaving.value = false
  }
}

/** 切换到系统主音量前先让用户决定是否恢复播放器的独立音量。 */
function selectVolumeTarget(value: unknown) {
  if (
    !isVolumeControlTarget(value) ||
    value === selectedVolumeTarget.value ||
    volumeTargetSaving.value
  ) {
    return
  }
  if (value === 'system') {
    pendingSystemVolumeSwitch.value = true
    return
  }
  void saveVolumeTarget(value)
}

/** 完成系统主音量切换，并按用户选择恢复当前播放器音量。 */
async function completeSystemVolumeSwitch(restoreApplicationVolume: boolean) {
  if (volumeTargetSaving.value) return
  pendingSystemVolumeSwitch.value = false
  if (!(await saveVolumeTarget('system')) || !restoreApplicationVolume) return

  try {
    await setCurrentMediaVolume(1)
  } catch (error) {
    notifyActionFailed(t('settings.taskbar.controls.volumeRestoreFailed'), error, {
      context: '恢复当前播放器音量失败',
    })
  }
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

onMounted(() => {
  void Promise.all([loadVisibility(), loadVolumeTarget()])
})
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-emerald-500">
      <CirclePlay />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.controls.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.controls.description') }}</ItemDescription>
    </ItemContent>
    <template #actions>
      <Switch
        :model-value="selectedVisibility.visible"
        :disabled="visibilitySaving"
        :aria-label="t('settings.taskbar.controls.visible')"
        @update:model-value="updateVisibility({ visible: $event })"
      />
    </template>

    <template #content>
      <FieldGroup>
        <Field :data-disabled="!selectedVisibility.visible">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.controls.buttons') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.controls.buttonsDescription')
            }}</FieldDescription>
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

        <Field
          v-if="selectedVisibility.volume"
          orientation="horizontal"
          :data-disabled="!selectedVisibility.visible"
        >
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.controls.volumeTarget') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.controls.volumeTargetDescription')
            }}</FieldDescription>
          </FieldContent>
          <Select
            :model-value="selectedVolumeTarget"
            :disabled="volumeTargetSaving || !selectedVisibility.visible"
            @update:model-value="selectVolumeTarget"
          >
            <SelectTrigger class="w-52" :aria-label="t('settings.taskbar.controls.volumeTarget')">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="application">
                  {{ t('settings.taskbar.controls.applicationVolume') }}
                </SelectItem>
                <SelectItem value="system">
                  {{ t('settings.taskbar.controls.systemVolume') }}
                </SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
  <VolumeTargetConfirmDialog
    v-model:open="volumeTargetDialogOpen"
    :disabled="volumeTargetSaving"
    @keep="completeSystemVolumeSwitch(false)"
    @restore="completeSystemVolumeSwitch(true)"
  />
</template>
