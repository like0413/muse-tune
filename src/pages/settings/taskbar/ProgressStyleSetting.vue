<script setup lang="ts">
import { Activity } from '@lucide/vue'
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
  DEFAULT_TASKBAR_PROGRESS_STYLE,
  getTaskbarProgressStyle,
  isTaskbarProgressStyle,
  setTaskbarProgressStyle,
  type TaskbarProgressStyle,
} from '@/features/settings/progress-style'

const progressStyleOptions = [
  { value: 'bottom', label: '底部横条' },
  { value: 'vertical-gradient', label: '竖线渐变' },
] as const satisfies ReadonlyArray<{ value: TaskbarProgressStyle; label: string }>

const selectedProgressStyle = shallowRef<TaskbarProgressStyle>(DEFAULT_TASKBAR_PROGRESS_STYLE)
const progressStyleSaving = shallowRef(false)

/** 恢复已保存的播放进度样式。 */
async function loadProgressStyle() {
  try {
    selectedProgressStyle.value = await getTaskbarProgressStyle()
  } catch (error) {
    console.error('读取播放进度样式失败', error)
  }
}

/** 保存播放进度样式，并在失败时恢复原来的持久化值与任务栏显示。 */
async function selectProgressStyle(value: unknown) {
  if (
    progressStyleSaving.value ||
    !isTaskbarProgressStyle(value) ||
    value === selectedProgressStyle.value
  ) {
    return
  }

  const previousStyle = selectedProgressStyle.value
  selectedProgressStyle.value = value
  progressStyleSaving.value = true

  try {
    await setTaskbarProgressStyle(value)
  } catch (error) {
    selectedProgressStyle.value = previousStyle
    console.error('保存播放进度样式失败', error)
    try {
      await setTaskbarProgressStyle(previousStyle)
    } catch (rollbackError) {
      console.error('恢复之前的播放进度样式失败', rollbackError)
    }
  } finally {
    progressStyleSaving.value = false
  }
}

onMounted(loadProgressStyle)
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-rose-500">
      <Activity />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>播放进度样式</ItemTitle>
      <ItemDescription>选择播放进度在任务栏播放器中的呈现方式</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Tabs :model-value="selectedProgressStyle" @update:model-value="selectProgressStyle">
        <TabsList aria-label="播放进度样式">
          <TabsTrigger
            v-for="option in progressStyleOptions"
            :key="option.value"
            :value="option.value"
            :disabled="progressStyleSaving"
          >
            {{ option.label }}
          </TabsTrigger>
        </TabsList>
      </Tabs>
    </ItemActions>
  </Item>
</template>
