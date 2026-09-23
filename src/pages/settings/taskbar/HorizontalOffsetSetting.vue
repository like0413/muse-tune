<script setup lang="ts">
import { ArrowLeftRight } from '@lucide/vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import {
  NumberField,
  NumberFieldContent,
  NumberFieldDecrement,
  NumberFieldIncrement,
  NumberFieldInput,
} from '@/components/ui/number-field'
import { notifySettingSaveFailed, reportBackgroundFailure } from '@/features/feedback/errors'
import { DEFAULT_TASKBAR_HORIZONTAL_OFFSET } from '@/features/settings/defaults'
import {
  applyTaskbarHorizontalOffset,
  getTaskbarHorizontalOffset,
  normalizeTaskbarHorizontalOffset,
  setTaskbarHorizontalOffset,
  TASKBAR_HORIZONTAL_OFFSET_MAX,
  TASKBAR_HORIZONTAL_OFFSET_MIN,
} from '@/features/settings/horizontal-offset'

const { t } = useI18n({ useScope: 'global' })

const selectedOffset = shallowRef(DEFAULT_TASKBAR_HORIZONTAL_OFFSET)
const committedOffset = shallowRef(DEFAULT_TASKBAR_HORIZONTAL_OFFSET)
const ready = shallowRef(false)
const saving = shallowRef(false)

/** 恢复已保存的水平偏移。 */
async function loadOffset() {
  try {
    const offset = await getTaskbarHorizontalOffset()
    selectedOffset.value = offset
    committedOffset.value = offset
  } catch (error) {
    reportBackgroundFailure('读取任务栏水平偏移失败', error)
  } finally {
    ready.value = true
  }
}

/**
 * 串行保存用户连续输入的最新值，避免多个 IPC 和存储写入乱序覆盖。
 * 保存期间仍允许继续增减；循环会跳过中间值并提交最新值。
 */
async function saveLatestOffset() {
  if (saving.value) return
  saving.value = true

  try {
    while (selectedOffset.value !== committedOffset.value) {
      const nextOffset = selectedOffset.value
      try {
        await setTaskbarHorizontalOffset(nextOffset)
        committedOffset.value = nextOffset
      } catch (error) {
        selectedOffset.value = committedOffset.value
        notifySettingSaveFailed(t('settings.taskbar.horizontalOffset.title'), error)
        try {
          await applyTaskbarHorizontalOffset(committedOffset.value)
        } catch (rollbackError) {
          reportBackgroundFailure('恢复之前的任务栏水平偏移失败', rollbackError)
        }
        return
      }
    }
  } finally {
    saving.value = false
  }
}

/** 接受数字输入框产生的有效整数，并触发串行保存。 */
function updateOffset(value: number | undefined) {
  if (!ready.value) return
  const offset = normalizeTaskbarHorizontalOffset(value)
  if (offset === undefined || offset === selectedOffset.value) return

  selectedOffset.value = offset
  void saveLatestOffset()
}

onMounted(loadOffset)
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-sky-500">
      <ArrowLeftRight />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.horizontalOffset.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.horizontalOffset.description') }}</ItemDescription>
    </ItemContent>
    <ItemActions>
      <NumberField
        class="w-32"
        :model-value="selectedOffset"
        :min="TASKBAR_HORIZONTAL_OFFSET_MIN"
        :max="TASKBAR_HORIZONTAL_OFFSET_MAX"
        :step="1"
        :disabled="!ready"
        :format-options="{ signDisplay: 'exceptZero' }"
        @update:model-value="updateOffset"
      >
        <NumberFieldContent>
          <NumberFieldDecrement />
          <NumberFieldInput :aria-label="t('settings.taskbar.horizontalOffset.title')" />
          <NumberFieldIncrement />
        </NumberFieldContent>
      </NumberField>
    </ItemActions>
  </Item>
</template>
