<script setup lang="ts">
import { PanelTop } from '@lucide/vue'
import { onMounted, shallowRef } from 'vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  applyTaskbarPlacement,
  getTaskbarPlacement,
  isTaskbarPlacement,
  setTaskbarPlacement,
  type TaskbarPlacement,
} from '@/lib/settings'

const placementOptions = [
  { value: 'left', label: '左侧' },
  { value: 'auto', label: '自动' },
  { value: 'right', label: '右侧' },
] as const satisfies ReadonlyArray<{ value: TaskbarPlacement; label: string }>

const selectedPlacement = shallowRef<TaskbarPlacement>('auto')
const placementSaving = shallowRef(false)

/** 恢复已保存的播放器位置。 */
async function loadPlacement() {
  try {
    selectedPlacement.value = await getTaskbarPlacement()
  } catch (error) {
    console.error('读取播放器位置失败', error)
  }
}

/** 保存选择并让任务栏播放器立即重新定位。 */
async function selectPlacement(value: unknown) {
  if (placementSaving.value || !isTaskbarPlacement(value) || value === selectedPlacement.value) {
    return
  }

  const previousPlacement = selectedPlacement.value
  selectedPlacement.value = value
  placementSaving.value = true

  try {
    await setTaskbarPlacement(value)
  } catch (error) {
    selectedPlacement.value = previousPlacement
    console.error('切换播放器位置失败', error)
    try {
      await applyTaskbarPlacement(previousPlacement)
    } catch (rollbackError) {
      console.error('恢复之前的播放器位置失败', rollbackError)
    }
  } finally {
    placementSaving.value = false
  }
}

onMounted(loadPlacement)
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-violet-500">
      <PanelTop />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>播放器位置</ItemTitle>
      <ItemDescription>自动模式会根据任务栏设置自动切换位置</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Tabs :model-value="selectedPlacement" @update:model-value="selectPlacement">
        <TabsList aria-label="播放器位置">
          <TabsTrigger
            v-for="option in placementOptions"
            :key="option.value"
            :value="option.value"
            :disabled="placementSaving"
          >
            {{ option.label }}
          </TabsTrigger>
        </TabsList>
      </Tabs>
    </ItemActions>
  </Item>
</template>
