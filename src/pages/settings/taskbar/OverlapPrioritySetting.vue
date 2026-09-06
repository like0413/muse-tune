<script setup lang="ts">
import { Layers2 } from '@lucide/vue'
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
  applyTaskbarOverlapPriority,
  getTaskbarOverlapPriority,
  isTaskbarOverlapPriority,
  setTaskbarOverlapPriority,
  type TaskbarOverlapPriority,
} from '@/features/settings/overlap-priority'

const overlapPriorityOptions = [
  { value: 'bar', label: 'MuseBar 优先' },
  { value: 'taskbar', label: '任务栏优先' },
] as const satisfies ReadonlyArray<{ value: TaskbarOverlapPriority; label: string }>

const selectedOverlapPriority = shallowRef<TaskbarOverlapPriority>('bar')
const overlapPrioritySaving = shallowRef(false)

/** 恢复已保存的遮挡优先级。 */
async function loadOverlapPriority() {
  try {
    selectedOverlapPriority.value = await getTaskbarOverlapPriority()
  } catch (error) {
    console.error('读取遮挡优先级失败', error)
  }
}

/** 保存选择并让任务栏播放器立即切换元素遮挡策略。 */
async function selectOverlapPriority(value: unknown) {
  if (
    overlapPrioritySaving.value ||
    !isTaskbarOverlapPriority(value) ||
    value === selectedOverlapPriority.value
  ) {
    return
  }

  const previousPriority = selectedOverlapPriority.value
  selectedOverlapPriority.value = value
  overlapPrioritySaving.value = true

  try {
    await setTaskbarOverlapPriority(value)
  } catch (error) {
    selectedOverlapPriority.value = previousPriority
    console.error('切换遮挡优先级失败', error)
    try {
      await applyTaskbarOverlapPriority(previousPriority)
    } catch (rollbackError) {
      console.error('恢复之前的遮挡优先级失败', rollbackError)
    }
  } finally {
    overlapPrioritySaving.value = false
  }
}

onMounted(loadOverlapPriority)
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-amber-500">
      <Layers2 />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>遮挡优先级</ItemTitle>
      <ItemDescription>空间不足时，MuseBar与任务栏元素谁显示在上方</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Tabs :model-value="selectedOverlapPriority" @update:model-value="selectOverlapPriority">
        <TabsList aria-label="元素显示顺序">
          <TabsTrigger
            v-for="option in overlapPriorityOptions"
            :key="option.value"
            :value="option.value"
            :disabled="overlapPrioritySaving"
          >
            {{ option.label }}
          </TabsTrigger>
        </TabsList>
      </Tabs>
    </ItemActions>
  </Item>
</template>
