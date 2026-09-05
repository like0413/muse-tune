<script setup lang="ts">
import { Blend } from '@lucide/vue'
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
  isTaskbarMaterial,
  setTaskbarMaterial,
  type TaskbarMaterial,
} from '@/lib/settings'

const materialOptions = [
  { value: 'normal', label: '正常' },
  { value: 'transparent', label: '透明' },
  { value: 'acrylic', label: '亚克力' },
] as const satisfies ReadonlyArray<{ value: TaskbarMaterial; label: string }>

const selectedMaterial = shallowRef<TaskbarMaterial>('normal')

/** 恢复已保存的窗口材质。 */
async function loadMaterial() {
  try {
    selectedMaterial.value = await getTaskbarMaterial()
  } catch (error) {
    console.error('读取任务栏窗口材质失败', error)
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

onMounted(loadMaterial)
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
  </div>
</template>
