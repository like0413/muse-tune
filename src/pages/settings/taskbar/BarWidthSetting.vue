<script setup lang="ts">
import { Ruler } from '@lucide/vue'
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
  applyTaskbarWidth,
  getTaskbarWidth,
  normalizeTaskbarWidth,
  setTaskbarWidth,
  TASKBAR_WIDTH_MAX,
  TASKBAR_WIDTH_MIN,
} from '@/features/settings/bar-width'

const WIDTH_PREVIEW_INTERVAL_MS = 50
const selectedWidth = shallowRef(TASKBAR_WIDTH_MAX)
const committedWidth = shallowRef(TASKBAR_WIDTH_MAX)
const widthSaving = shallowRef(false)

/** 恢复已保存的 bar 基准宽度。 */
async function loadWidth() {
  try {
    const width = await getTaskbarWidth()
    selectedWidth.value = width
    committedWidth.value = width
  } catch (error) {
    console.error('读取 bar 宽度失败', error)
  }
}

/** 读取 Slider 的单个有效宽度值。 */
function getWidthValue(values: number[] | undefined): number | undefined {
  return normalizeTaskbarWidth(values?.[0])
}

/** 限频应用拖动预览；关闭尾调用可防止旧宽度覆盖最终提交值。 */
const previewWidth = useThrottleFn(
  (width: number) => {
    applyTaskbarWidth(width).catch((error) => {
      console.error('预览 bar 宽度失败', error)
    })
  },
  WIDTH_PREVIEW_INTERVAL_MS,
  true,
  false,
)

/** 更新 Slider 状态并预览宽度，不写入持久化存储。 */
function updateWidth(values: number[] | undefined) {
  if (widthSaving.value) return

  const width = getWidthValue(values)
  if (width === undefined) return

  selectedWidth.value = width
  previewWidth(width)
}

/** 在交互结束时应用并持久化最终宽度。 */
async function commitWidth(values: number[]) {
  if (widthSaving.value) return

  const width = getWidthValue(values)
  if (width === undefined) return

  widthSaving.value = true
  try {
    await setTaskbarWidth(width)
    committedWidth.value = width
  } catch (error) {
    console.error('保存 bar 宽度失败', error)
    const previousWidth = committedWidth.value
    selectedWidth.value = previousWidth
    try {
      await applyTaskbarWidth(previousWidth)
    } catch (rollbackError) {
      console.error('恢复之前的 bar 宽度失败', rollbackError)
    }
  } finally {
    widthSaving.value = false
  }
}

onMounted(loadWidth)
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-cyan-500">
      <Ruler />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>bar 宽度</ItemTitle>
      <ItemDescription>设置未被任务栏元素裁剪时的完整宽度</ItemDescription>
    </ItemContent>
    <ItemActions class="w-56">
      <Slider
        :model-value="[selectedWidth]"
        :min="TASKBAR_WIDTH_MIN"
        :max="TASKBAR_WIDTH_MAX"
        :step="1"
        :disabled="widthSaving"
        aria-label="bar 宽度"
        @update:model-value="updateWidth"
        @value-commit="commitWidth"
      />
      <output class="text-muted-foreground w-14 text-right text-xs tabular-nums">
        {{ selectedWidth }}px
      </output>
    </ItemActions>
  </Item>
</template>
