<script setup lang="ts">
import { Blend } from '@lucide/vue'
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
import {
  getTaskbarBackgroundTransparency,
  normalizeTaskbarBackgroundTransparency,
  previewTaskbarBackgroundTransparency,
  setTaskbarBackgroundTransparency,
  TASKBAR_TRANSPARENCY_MAX,
  TASKBAR_TRANSPARENCY_MIN,
} from '@/features/settings/background-transparency'

const selectedBackgroundTransparency = shallowRef(0)
const committedBackgroundTransparency = shallowRef(0)
const backgroundTransparencySaving = shallowRef(false)

/** 恢复已保存的背景透明度。 */
async function loadBackgroundTransparency() {
  try {
    const transparency = await getTaskbarBackgroundTransparency()
    selectedBackgroundTransparency.value = transparency
    committedBackgroundTransparency.value = transparency
  } catch (error) {
    console.error('读取任务栏背景透明度失败', error)
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
  if (backgroundTransparencySaving.value) return

  const transparency = getTransparencyValue(values)
  if (transparency === undefined) return

  selectedBackgroundTransparency.value = transparency
  previewTransparency(transparency)
}

/** 在交互结束时持久化最终透明度。 */
async function commitBackgroundTransparency(values: number[]) {
  if (backgroundTransparencySaving.value) return

  const transparency = getTransparencyValue(values)
  if (transparency === undefined) return

  backgroundTransparencySaving.value = true
  previewTransparency(transparency)
  try {
    await setTaskbarBackgroundTransparency(transparency)
    committedBackgroundTransparency.value = transparency
  } catch (error) {
    console.error('保存任务栏背景透明度失败', error)
    const committedTransparency = committedBackgroundTransparency.value
    selectedBackgroundTransparency.value = committedTransparency
    previewTransparency(committedTransparency)
    try {
      await previewTaskbarBackgroundTransparency(committedTransparency)
    } catch (rollbackError) {
      console.error('恢复之前的背景透明度失败', rollbackError)
    }
  } finally {
    backgroundTransparencySaving.value = false
  }
}

onMounted(loadBackgroundTransparency)
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-sky-500">
      <Blend />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>背景透明度</ItemTitle>
      <ItemDescription>仅调组件背景，文字与控件保持清晰</ItemDescription>
    </ItemContent>
    <ItemActions class="w-56">
      <Slider
        :model-value="[selectedBackgroundTransparency]"
        :min="TASKBAR_TRANSPARENCY_MIN"
        :max="TASKBAR_TRANSPARENCY_MAX"
        :step="1"
        :disabled="backgroundTransparencySaving"
        aria-label="背景透明度"
        @update:model-value="updateBackgroundTransparency"
        @value-commit="commitBackgroundTransparency"
      />
      <output class="text-muted-foreground w-10 text-right text-xs tabular-nums">
        {{ selectedBackgroundTransparency }}%
      </output>
    </ItemActions>
  </Item>
</template>
