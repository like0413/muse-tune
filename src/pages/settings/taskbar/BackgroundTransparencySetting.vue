<script setup lang="ts">
import { Blend } from '@lucide/vue'
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
import { notifySettingSaveFailed, reportBackgroundFailure } from '@/features/feedback/errors'
import { useEventState } from '@/features/ipc/useEventState'
import {
  DEFAULT_TASKBAR_BACKGROUND_STYLE,
  getTaskbarBackgroundStyle,
  listenTaskbarBackgroundStyleChange,
} from '@/features/settings/background-style'
import {
  getTaskbarBackgroundTransparency,
  normalizeTaskbarBackgroundTransparency,
  previewTaskbarBackgroundTransparency,
  setTaskbarBackgroundTransparency,
  TASKBAR_TRANSPARENCY_MAX,
  TASKBAR_TRANSPARENCY_MIN,
} from '@/features/settings/background-transparency'
import { DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY } from '@/features/settings/defaults'

const { t } = useI18n({ useScope: 'global' })

const selectedBackgroundTransparency = shallowRef(DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY)
const committedBackgroundTransparency = shallowRef(DEFAULT_TASKBAR_BACKGROUND_TRANSPARENCY)
const backgroundTransparencySaving = shallowRef(false)
/** 当前背景样式；订阅变更以确保切换模式后透明度控件立即更新。 */
const backgroundStyle = useEventState(
  {
    read: getTaskbarBackgroundStyle,
    subscribe: listenTaskbarBackgroundStyleChange,
    failureMessage: '读取任务栏背景样式失败',
  },
  DEFAULT_TASKBAR_BACKGROUND_STYLE,
)
const transparencyDisabled = computed(
  () => backgroundTransparencySaving.value || backgroundStyle.value !== 'theme',
)

/** 恢复已保存的背景透明度。 */
async function loadBackgroundTransparency() {
  try {
    const transparency = await getTaskbarBackgroundTransparency()
    selectedBackgroundTransparency.value = transparency
    committedBackgroundTransparency.value = transparency
  } catch (error) {
    reportBackgroundFailure('读取任务栏背景透明度失败', error)
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
      reportBackgroundFailure('预览任务栏背景透明度失败', error)
    })
  },
  32,
  true,
  true,
)

/** 更新 Slider 状态并实时预览，不写入持久化存储。 */
function updateBackgroundTransparency(values: number[] | undefined) {
  if (transparencyDisabled.value) return

  const transparency = getTransparencyValue(values)
  if (transparency === undefined) return

  selectedBackgroundTransparency.value = transparency
  previewTransparency(transparency)
}

/** 在交互结束时持久化最终透明度。 */
async function commitBackgroundTransparency(values: number[]) {
  if (transparencyDisabled.value) return

  const transparency = getTransparencyValue(values)
  if (transparency === undefined) return

  backgroundTransparencySaving.value = true
  previewTransparency(transparency)
  try {
    await setTaskbarBackgroundTransparency(transparency)
    committedBackgroundTransparency.value = transparency
  } catch (error) {
    notifySettingSaveFailed(t('settings.taskbar.transparency.title'), error)
    const committedTransparency = committedBackgroundTransparency.value
    selectedBackgroundTransparency.value = committedTransparency
    previewTransparency(committedTransparency)
    try {
      await previewTaskbarBackgroundTransparency(committedTransparency)
    } catch (rollbackError) {
      reportBackgroundFailure('恢复之前的背景透明度失败', rollbackError)
    }
  } finally {
    backgroundTransparencySaving.value = false
  }
}

onMounted(() => void loadBackgroundTransparency())
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-sky-500">
      <Blend />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.transparency.title') }}</ItemTitle>
      <ItemDescription>{{
        t(
          backgroundStyle === 'theme'
            ? 'settings.taskbar.transparency.description'
            : 'settings.taskbar.transparency.coverDisabled',
        )
      }}</ItemDescription>
    </ItemContent>
    <ItemActions class="w-56">
      <Slider
        :model-value="[selectedBackgroundTransparency]"
        :min="TASKBAR_TRANSPARENCY_MIN"
        :max="TASKBAR_TRANSPARENCY_MAX"
        :step="1"
        :disabled="transparencyDisabled"
        :aria-label="t('settings.taskbar.transparency.title')"
        @update:model-value="updateBackgroundTransparency"
        @value-commit="commitBackgroundTransparency"
      />
      <output class="text-muted-foreground w-10 text-right text-xs tabular-nums">
        {{ selectedBackgroundTransparency }}%
      </output>
    </ItemActions>
  </Item>
</template>
