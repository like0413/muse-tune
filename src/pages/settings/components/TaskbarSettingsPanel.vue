<script setup lang="ts">
import { Blend, PanelTop } from '@lucide/vue'
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
  getTaskbarMaterial,
  getTaskbarPlacement,
  isTaskbarMaterial,
  isTaskbarPlacement,
  setTaskbarMaterial,
  setTaskbarPlacement,
  type TaskbarMaterial,
  type TaskbarPlacement,
} from '@/lib/settings'

const materialOptions = [
  { value: 'normal', label: '正常' },
  { value: 'transparent', label: '透明' },
  { value: 'acrylic', label: '亚克力' },
] as const satisfies ReadonlyArray<{ value: TaskbarMaterial; label: string }>

const placementOptions = [
  { value: 'left', label: '左侧' },
  { value: 'right', label: '右侧' },
  { value: 'auto', label: '自动' },
] as const satisfies ReadonlyArray<{ value: TaskbarPlacement; label: string }>

const selectedMaterial = shallowRef<TaskbarMaterial>('normal')
const selectedPlacement = shallowRef<TaskbarPlacement>('auto')

/** 恢复已保存的窗口材质。 */
async function loadMaterial() {
  try {
    selectedMaterial.value = await getTaskbarMaterial()
  } catch (error) {
    console.error('读取任务栏窗口材质失败', error)
  }
}

/** 恢复已保存的播放器位置。 */
async function loadPlacement() {
  try {
    selectedPlacement.value = await getTaskbarPlacement()
  } catch (error) {
    console.error('读取播放器位置失败', error)
  }
}

/** 保存选择并让任务栏窗口立即切换材质。 */
async function selectMaterial(value: unknown) {
  if (!isTaskbarMaterial(value) || value === selectedMaterial.value) {
    return
  }

  const previousMaterial = selectedMaterial.value
  selectedMaterial.value = value

  try {
    await setTaskbarMaterial(value)
  } catch (error) {
    selectedMaterial.value = previousMaterial
    console.error('切换任务栏窗口材质失败', error)
  }
}

/** 保存选择并让任务栏播放器立即重新定位。 */
async function selectPlacement(value: unknown) {
  if (!isTaskbarPlacement(value) || value === selectedPlacement.value) {
    return
  }

  const previousPlacement = selectedPlacement.value
  selectedPlacement.value = value

  try {
    await setTaskbarPlacement(value)
  } catch (error) {
    selectedPlacement.value = previousPlacement
    console.error('切换播放器位置失败', error)
  }
}

onMounted(loadMaterial)
onMounted(loadPlacement)
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <Item variant="outline">
      <ItemMedia variant="icon" class="icon-tone-sky-500">
        <Blend />
      </ItemMedia>
      <ItemContent>
        <ItemTitle>窗口材质</ItemTitle>
        <ItemDescription>任务栏播放器的背景呈现方式</ItemDescription>
      </ItemContent>
      <ItemActions>
        <Tabs :model-value="selectedMaterial" @update:model-value="selectMaterial">
          <TabsList aria-label="窗口材质">
            <TabsTrigger
              v-for="option in materialOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </TabsTrigger>
          </TabsList>
        </Tabs>
      </ItemActions>
    </Item>

    <Item variant="outline">
      <ItemMedia variant="icon" class="icon-tone-violet-500">
        <PanelTop />
      </ItemMedia>
      <ItemContent>
        <ItemTitle>播放器位置</ItemTitle>
        <ItemDescription>自动模式会避开 Windows 任务栏按钮所在一侧</ItemDescription>
      </ItemContent>
      <ItemActions>
        <Tabs :model-value="selectedPlacement" @update:model-value="selectPlacement">
          <TabsList aria-label="播放器位置">
            <TabsTrigger
              v-for="option in placementOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </TabsTrigger>
          </TabsList>
        </Tabs>
      </ItemActions>
    </Item>
  </div>
</template>
