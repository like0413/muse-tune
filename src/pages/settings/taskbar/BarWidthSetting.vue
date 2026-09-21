<script setup lang="ts">
import { Ruler } from '@lucide/vue'
import { useThrottleFn } from '@vueuse/core'

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
import { notifySettingSaveFailed, reportBackgroundFailure } from '@/features/feedback/errors'
import {
  applyTaskbarWidth,
  getTaskbarWidth,
  getTaskbarWidthMode,
  getTaskbarWidthPreset,
  normalizeTaskbarWidth,
  setTaskbarWidth,
  TASKBAR_WIDTH_MAX,
  TASKBAR_WIDTH_MIN,
  TASKBAR_WIDTH_PRESETS,
  type TaskbarWidthMode,
  type TaskbarWidthPreset,
} from '@/features/settings/bar-width'
import { DEFAULT_TASKBAR_WIDTH } from '@/features/settings/defaults'

const { t } = useI18n({ useScope: 'global' })

/** 宽度页签：三个固定预设、自由调整，以及自动占满任务栏空白。 */
type WidthOption = TaskbarWidthPreset | 'auto'

const WIDTH_PREVIEW_INTERVAL_MS = 50
const selectedWidth = shallowRef(DEFAULT_TASKBAR_WIDTH)
const committedWidth = shallowRef(DEFAULT_TASKBAR_WIDTH)
const committedMode = shallowRef<TaskbarWidthMode>('fixed')
const widthSaving = shallowRef(false)
const selectedPreset = shallowRef<WidthOption>('wide')
const widthPresetOptions = computed(
  () =>
    [
      { value: 'compact', label: t('settings.taskbar.width.compact') },
      { value: 'standard', label: t('settings.taskbar.width.standard') },
      { value: 'wide', label: t('settings.taskbar.width.wide') },
      { value: 'custom', label: t('settings.taskbar.width.custom') },
      { value: 'auto', label: t('settings.taskbar.width.auto') },
    ] as const,
)

/** 恢复已保存的 bar 宽度与宽度模式。 */
async function loadWidth() {
  try {
    const [width, mode] = await Promise.all([getTaskbarWidth(), getTaskbarWidthMode()])
    selectedWidth.value = width
    committedWidth.value = width
    committedMode.value = mode
    selectedPreset.value = mode === 'auto' ? 'auto' : getTaskbarWidthPreset(width)
  } catch (error) {
    reportBackgroundFailure('读取 bar 宽度失败', error)
  }
}

/** 切换宽度预设；自由调整只展开滑块，自适应直接切到原生自适应模式。 */
async function selectPreset(value: string | number) {
  if (!(typeof value === 'string' && widthPresetOptions.value.some((item) => item.value === value)))
    return
  const preset = value as WidthOption
  selectedPreset.value = preset
  if (preset === 'auto') {
    // 保留已保存的固定宽度，切回固定宽度时可以继续使用。
    await commitWidth([selectedWidth.value], 'auto')
    return
  }
  if (preset === 'custom') return
  const width = TASKBAR_WIDTH_PRESETS[preset]
  selectedWidth.value = width
  await commitWidth([width], 'fixed')
}

/** 读取 Slider 的单个有效宽度值。 */
function getWidthValue(values: number[] | undefined): number | undefined {
  return normalizeTaskbarWidth(values?.[0])
}

/** 限频应用拖动预览；关闭尾调用可防止旧宽度覆盖最终提交值。 */
const previewWidth = useThrottleFn(
  (width: number) => {
    applyTaskbarWidth(width, 'fixed').catch((error) => {
      reportBackgroundFailure('预览 bar 宽度失败', error)
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

/** 在交互结束时应用并持久化最终宽度与模式。 */
async function commitWidth(values: number[], mode: TaskbarWidthMode = 'fixed') {
  if (widthSaving.value) return

  const width = getWidthValue(values)
  if (width === undefined) return

  widthSaving.value = true
  try {
    await setTaskbarWidth(width, mode)
    committedWidth.value = width
    committedMode.value = mode
  } catch (error) {
    notifySettingSaveFailed(t('settings.taskbar.width.title'), error)
    selectedWidth.value = committedWidth.value
    selectedPreset.value =
      committedMode.value === 'auto' ? 'auto' : getTaskbarWidthPreset(committedWidth.value)
    try {
      await applyTaskbarWidth(committedWidth.value, committedMode.value)
    } catch (rollbackError) {
      reportBackgroundFailure('恢复之前的 bar 宽度失败', rollbackError)
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
