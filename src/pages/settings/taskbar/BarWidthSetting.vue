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
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  applyTaskbarWidth,
  getTaskbarWidth,
  getTaskbarWidthPreset,
  normalizeTaskbarWidth,
  setTaskbarWidth,
  TASKBAR_WIDTH_MAX,
  TASKBAR_WIDTH_MIN,
  TASKBAR_WIDTH_PRESETS,
  type TaskbarWidthPreset,
} from '@/features/settings/bar-width'

const { t } = useI18n({ useScope: 'global' })

const WIDTH_PREVIEW_INTERVAL_MS = 50
const selectedWidth = shallowRef(TASKBAR_WIDTH_MAX)
const committedWidth = shallowRef(TASKBAR_WIDTH_MAX)
const widthSaving = shallowRef(false)
const selectedPreset = shallowRef<TaskbarWidthPreset>('wide')
const widthPresetOptions = computed(
  () =>
    [
      { value: 'compact', label: t('settings.taskbar.width.compact') },
      { value: 'standard', label: t('settings.taskbar.width.standard') },
      { value: 'wide', label: t('settings.taskbar.width.wide') },
      { value: 'custom', label: t('settings.taskbar.width.custom') },
    ] as const,
)

/** 恢复已保存的 bar 基准宽度。 */
async function loadWidth() {
  try {
    const width = await getTaskbarWidth()
    selectedWidth.value = width
    committedWidth.value = width
    selectedPreset.value = getTaskbarWidthPreset(width)
  } catch (error) {
    console.error('读取 bar 宽度失败', error)
  }
}

/** 切换宽度预设；自由调整只展开滑块，不主动改变当前宽度。 */
async function selectPreset(value: string | number) {
  if (!(typeof value === 'string' && widthPresetOptions.value.some((item) => item.value === value)))
    return
  const preset = value as TaskbarWidthPreset
  selectedPreset.value = preset
  if (preset === 'custom') return
  const width = TASKBAR_WIDTH_PRESETS[preset]
  selectedWidth.value = width
  await commitWidth([width])
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
  selectedPreset.value = 'custom'
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
    notifySettingSaveFailed(t('settings.taskbar.width.title'), error)
    const previousWidth = committedWidth.value
    selectedWidth.value = previousWidth
    selectedPreset.value = getTaskbarWidthPreset(previousWidth)
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
      <ItemTitle>{{ t('settings.taskbar.width.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.width.description') }}</ItemDescription>
    </ItemContent>
    <ItemActions class="flex-col items-end gap-2">
      <Tabs :model-value="selectedPreset" @update:model-value="selectPreset">
        <TabsList>
          <TabsTrigger
            v-for="option in widthPresetOptions"
            :key="option.value"
            :value="option.value"
            :disabled="widthSaving"
          >
            {{ option.label }}
          </TabsTrigger>
        </TabsList>
      </Tabs>
      <div v-if="selectedPreset === 'custom'" class="flex w-72 items-center gap-3">
        <Slider
          :model-value="[selectedWidth]"
          :min="TASKBAR_WIDTH_MIN"
          :max="TASKBAR_WIDTH_MAX"
          :step="1"
          :disabled="widthSaving"
          :aria-label="t('settings.taskbar.width.custom')"
          @update:model-value="updateWidth"
          @value-commit="commitWidth"
        />
        <output class="text-muted-foreground w-14 text-right text-xs tabular-nums">
          {{ selectedWidth }}px
        </output>
      </div>
    </ItemActions>
  </Item>
</template>
