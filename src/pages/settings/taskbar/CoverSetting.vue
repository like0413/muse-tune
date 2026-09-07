<script setup lang="ts">
import { Image } from '@lucide/vue'
import { onMounted, shallowRef } from 'vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldTitle,
} from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  DEFAULT_TASKBAR_COVER_APPEARANCE,
  getTaskbarCoverAppearance,
  isTaskbarCoverShape,
  setTaskbarCoverAppearance,
  type TaskbarCoverAppearance,
} from '@/features/settings/cover'

const shapeOptions = [
  { value: 'square', label: '方形' },
  { value: 'rounded', label: '圆角' },
  { value: 'circle', label: '圆形' },
] as const

const selectedAppearance = shallowRef<TaskbarCoverAppearance>({
  ...DEFAULT_TASKBAR_COVER_APPEARANCE,
})
const committedAppearance = shallowRef<TaskbarCoverAppearance>({
  ...DEFAULT_TASKBAR_COVER_APPEARANCE,
})
const appearanceSaving = shallowRef(false)

/** 恢复已保存的封面配置。 */
async function loadAppearance() {
  try {
    const appearance = await getTaskbarCoverAppearance()
    selectedAppearance.value = appearance
    committedAppearance.value = { ...appearance }
  } catch (error) {
    console.error('读取封面配置失败', error)
  }
}

/** 合并并持久化一次封面配置变更，失败时恢复最近成功值。 */
async function updateAppearance(patch: Partial<TaskbarCoverAppearance>) {
  if (appearanceSaving.value) return

  const nextAppearance = { ...selectedAppearance.value, ...patch }
  selectedAppearance.value = nextAppearance
  appearanceSaving.value = true
  try {
    await setTaskbarCoverAppearance(nextAppearance)
    committedAppearance.value = { ...nextAppearance }
  } catch (error) {
    selectedAppearance.value = { ...committedAppearance.value }
    console.error('保存封面配置失败', error)
  } finally {
    appearanceSaving.value = false
  }
}

/** 接收 Tabs 的外部值并更新封面形状。 */
function selectShape(value: string | number) {
  if (isTaskbarCoverShape(value)) void updateAppearance({ shape: value })
}

onMounted(loadAppearance)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-amber-500">
      <Image />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>封面调整</ItemTitle>
      <ItemDescription>设置封面的显示、形状与播放时旋转</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-cover-visible">显示封面</FieldLabel>
            <FieldDescription>控制任务栏播放器中的封面区域</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-cover-visible"
            :model-value="selectedAppearance.visible"
            :disabled="appearanceSaving"
            @update:model-value="updateAppearance({ visible: $event })"
          />
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedAppearance.visible">
          <FieldContent>
            <FieldTitle>封面形状</FieldTitle>
            <FieldDescription>选择方形、圆角或圆形封面</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedAppearance.shape" @update:model-value="selectShape">
            <TabsList aria-label="封面形状">
              <TabsTrigger
                v-for="option in shapeOptions"
                :key="option.value"
                :value="option.value"
                :disabled="appearanceSaving || !selectedAppearance.visible"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>

        <Field
          v-if="selectedAppearance.shape === 'circle'"
          orientation="horizontal"
          :data-disabled="!selectedAppearance.visible"
        >
          <FieldContent>
            <FieldLabel for="taskbar-cover-rotate">播放时旋转</FieldLabel>
            <FieldDescription>播放时持续旋转，暂停后停在当前位置</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-cover-rotate"
            :model-value="selectedAppearance.rotateWhenPlaying"
            :disabled="appearanceSaving || !selectedAppearance.visible"
            @update:model-value="updateAppearance({ rotateWhenPlaying: $event })"
          />
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedAppearance.visible">
          <FieldContent>
            <FieldLabel for="taskbar-cover-player-source">显示播放器来源</FieldLabel>
            <FieldDescription>在封面右下角显示当前播放器的小图标</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-cover-player-source"
            :model-value="selectedAppearance.showPlayerSource"
            :disabled="appearanceSaving || !selectedAppearance.visible"
            @update:model-value="updateAppearance({ showPlayerSource: $event })"
          />
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
