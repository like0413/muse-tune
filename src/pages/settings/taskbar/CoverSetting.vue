<script setup lang="ts">
import { Image } from '@lucide/vue'
import { onMounted, shallowRef } from 'vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Label } from '@/components/ui/label'
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
      <div class="grid gap-4">
        <div class="flex items-center justify-between gap-4">
          <Label for="taskbar-cover-visible">显示封面</Label>
          <Switch
            id="taskbar-cover-visible"
            :model-value="selectedAppearance.visible"
            :disabled="appearanceSaving"
            @update:model-value="updateAppearance({ visible: $event })"
          />
        </div>

        <div class="flex items-center justify-between gap-4">
          <span class="text-sm font-medium">封面形状</span>
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
        </div>

        <div
          v-if="selectedAppearance.shape === 'circle'"
          class="flex items-center justify-between gap-4"
        >
          <div class="grid gap-0.5">
            <Label for="taskbar-cover-rotate">播放时旋转</Label>
            <p class="text-muted-foreground text-xs">媒体播放状态接入后自动启停</p>
          </div>
          <Switch
            id="taskbar-cover-rotate"
            :model-value="selectedAppearance.rotateWhenPlaying"
            :disabled="appearanceSaving || !selectedAppearance.visible"
            @update:model-value="updateAppearance({ rotateWhenPlaying: $event })"
          />
        </div>
      </div>
    </template>
  </CollapsibleItem>
</template>
