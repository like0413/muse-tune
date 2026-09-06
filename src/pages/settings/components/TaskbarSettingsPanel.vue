<script setup lang="ts">
import { Blend, Layers2, PanelTop } from '@lucide/vue'
import { useThrottleFn } from '@vueuse/core'
import { onMounted, shallowRef } from 'vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Slider } from '@/components/ui/slider'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  getTaskbarBackgroundTransparency,
  getTaskbarOverlapPriority,
  getTaskbarPlacement,
  isTaskbarOverlapPriority,
  isTaskbarPlacement,
  normalizeTaskbarBackgroundTransparency,
  previewTaskbarBackgroundTransparency,
  setTaskbarBackgroundTransparency,
  setTaskbarOverlapPriority,
  setTaskbarPlacement,
  TASKBAR_TRANSPARENCY_MAX,
  TASKBAR_TRANSPARENCY_MIN,
  type TaskbarOverlapPriority,
  type TaskbarPlacement,
} from '@/lib/settings'

const placementOptions = [
  { value: 'left', label: '左侧' },
  { value: 'auto', label: '自动' },
  { value: 'right', label: '右侧' },
] as const satisfies ReadonlyArray<{ value: TaskbarPlacement; label: string }>

const overlapPriorityOptions = [
  { value: 'bar', label: '播放器优先' },
  { value: 'taskbar', label: '任务栏元素优先' },
] as const satisfies ReadonlyArray<{ value: TaskbarOverlapPriority; label: string }>

const selectedBackgroundTransparency = shallowRef(0)
const selectedPlacement = shallowRef<TaskbarPlacement>('auto')
const selectedOverlapPriority = shallowRef<TaskbarOverlapPriority>('bar')

/** 恢复已保存的背景透明度。 */
async function loadBackgroundTransparency() {
  try {
    selectedBackgroundTransparency.value = await getTaskbarBackgroundTransparency()
  } catch (error) {
    console.error('读取任务栏背景透明度失败', error)
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

/** 恢复已保存的遮挡优先级。 */
async function loadOverlapPriority() {
  try {
    selectedOverlapPriority.value = await getTaskbarOverlapPriority()
  } catch (error) {
    console.error('读取遮挡优先级失败', error)
  }
}

/** 读取 Slider 的单个有效透明度值。 */
function getTransparencyValue(values: number[] | undefined): number | undefined {
  return normalizeTaskbarBackgroundTransparency(values?.[0])
}

/** 限频发送拖动预览，避免为每个指针事件跨窗口通信。 */
const previewTransparency = useThrottleFn(
  (transparency: number) => {
    previewTaskbarBackgroundTransparency(transparency).catch((error) => {
      console.error('预览任务栏背景透明度失败', error)
    })
  },
  32,
  true,
  true,
)

/** 更新 Slider 状态并实时预览，不写入持久化存储。 */
function updateBackgroundTransparency(values: number[] | undefined) {
  const transparency = getTransparencyValue(values)
  if (transparency === undefined) {
    return
  }

  selectedBackgroundTransparency.value = transparency
  previewTransparency(transparency)
}

/** 在交互结束时持久化最终透明度。 */
async function commitBackgroundTransparency(values: number[]) {
  const transparency = getTransparencyValue(values)
  if (transparency === undefined) {
    return
  }

  try {
    await setTaskbarBackgroundTransparency(transparency)
  } catch (error) {
    console.error('保存任务栏背景透明度失败', error)
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

/** 保存选择并让任务栏播放器立即切换元素遮挡策略。 */
async function selectOverlapPriority(value: unknown) {
  if (!isTaskbarOverlapPriority(value) || value === selectedOverlapPriority.value) {
    return
  }

  const previousPriority = selectedOverlapPriority.value
  selectedOverlapPriority.value = value

  try {
    await setTaskbarOverlapPriority(value)
  } catch (error) {
    selectedOverlapPriority.value = previousPriority
    console.error('切换遮挡优先级失败', error)
  }
}

onMounted(loadBackgroundTransparency)
onMounted(loadPlacement)
onMounted(loadOverlapPriority)
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <Item>
      <ItemMedia class="icon-tone-sky-500">
        <Blend />
      </ItemMedia>
      <ItemContent>
        <ItemTitle>背景透明度</ItemTitle>
        <ItemDescription>仅调整播放器背景，文字与控件保持清晰</ItemDescription>
      </ItemContent>
      <ItemActions class="w-56">
        <Slider
          :model-value="[selectedBackgroundTransparency]"
          :min="TASKBAR_TRANSPARENCY_MIN"
          :max="TASKBAR_TRANSPARENCY_MAX"
          :step="1"
          aria-label="背景透明度"
          @update:model-value="updateBackgroundTransparency"
          @value-commit="commitBackgroundTransparency"
        />
        <output class="text-muted-foreground w-10 text-right text-xs tabular-nums">
          {{ selectedBackgroundTransparency }}%
        </output>
      </ItemActions>
    </Item>

    <Item>
      <ItemMedia class="icon-tone-violet-500">
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

    <Item>
      <ItemMedia class="icon-tone-amber-500">
        <Layers2 />
      </ItemMedia>
      <ItemContent>
        <ItemTitle>元素显示顺序</ItemTitle>
        <ItemDescription>空间不足时，播放器与任务栏元素谁显示在上方</ItemDescription>
      </ItemContent>
      <ItemActions>
        <Tabs :model-value="selectedOverlapPriority" @update:model-value="selectOverlapPriority">
          <TabsList aria-label="元素显示顺序">
            <TabsTrigger
              v-for="option in overlapPriorityOptions"
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
