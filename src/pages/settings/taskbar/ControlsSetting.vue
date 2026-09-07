<script setup lang="ts">
import { Gamepad2 } from '@lucide/vue'
import { onMounted, shallowRef } from 'vue'

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
import {
  DEFAULT_TASKBAR_CONTROLS_VISIBILITY,
  getTaskbarControlsVisibility,
  setTaskbarControlsVisibility,
  type TaskbarControlsVisibility,
} from '@/features/settings/controls'

const buttonOptions = [
  { key: 'previous', label: '上一曲' },
  { key: 'playPause', label: '播放 / 暂停' },
  { key: 'next', label: '下一曲' },
] as const

type ControlButtonKey = (typeof buttonOptions)[number]['key']

const selectedVisibility = shallowRef<TaskbarControlsVisibility>({
  ...DEFAULT_TASKBAR_CONTROLS_VISIBILITY,
})
const committedVisibility = shallowRef<TaskbarControlsVisibility>({
  ...DEFAULT_TASKBAR_CONTROLS_VISIBILITY,
})
const visibilitySaving = shallowRef(false)

/** 恢复已保存的控制按钮配置。 */
async function loadVisibility() {
  try {
    const visibility = await getTaskbarControlsVisibility()
    selectedVisibility.value = visibility
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
    console.error('保存控制按钮配置失败', error)
  } finally {
    visibilitySaving.value = false
  }
}

/** 将 Checkbox 值规范为布尔值并更新单个按钮。 */
function updateButton(key: ControlButtonKey, value: boolean | 'indeterminate') {
  void updateVisibility({ [key]: value === true })
}

onMounted(loadVisibility)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-violet-500">
      <Gamepad2 />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>控制按钮调整</ItemTitle>
      <ItemDescription>控制按钮组整体及各按钮的显示状态</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-controls-visible">显示控制按钮组</FieldLabel>
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
            <FieldTitle>显示的按钮</FieldTitle>
          </FieldContent>
          <div class="grid grid-cols-3 gap-2">
            <Label
              v-for="option in buttonOptions"
              :key="option.key"
              :for="`taskbar-control-${option.key}`"
              class="bg-muted/50 flex items-center gap-2 rounded-md border p-3"
            >
              <Checkbox
                :id="`taskbar-control-${option.key}`"
                :model-value="selectedVisibility[option.key]"
                :disabled="visibilitySaving || !selectedVisibility.visible"
                @update:model-value="updateButton(option.key, $event)"
              />
              <span class="truncate text-sm">{{ option.label }}</span>
            </Label>
          </div>
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
