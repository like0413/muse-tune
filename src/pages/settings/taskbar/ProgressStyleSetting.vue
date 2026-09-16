<script setup lang="ts">
import { Activity } from '@lucide/vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldTitle,
} from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_PROGRESS_POSITION,
  DEFAULT_TASKBAR_PROGRESS_STYLE,
  getTaskbarProgressPosition,
  getTaskbarProgressStyle,
  isTaskbarProgressPosition,
  isTaskbarProgressStyle,
  setTaskbarProgressPosition,
  setTaskbarProgressStyle,
  type TaskbarProgressPosition,
  type TaskbarProgressStyle,
} from '@/features/settings/progress-style'

const { t } = useI18n({ useScope: 'global' })

const progressStyleOptions = computed(
  () =>
    [
      { value: 'bottom', label: t('settings.taskbar.progress.bar') },
      { value: 'vertical-gradient', label: t('settings.taskbar.progress.gradient') },
    ] as const satisfies ReadonlyArray<{ value: TaskbarProgressStyle; label: string }>,
)
const progressPositionOptions = computed(
  () =>
    [
      { value: 'top', label: t('settings.taskbar.progress.top') },
      { value: 'bottom', label: t('settings.taskbar.progress.bottom') },
    ] as const satisfies ReadonlyArray<{ value: TaskbarProgressPosition; label: string }>,
)

const selectedProgressStyle = shallowRef<TaskbarProgressStyle>(DEFAULT_TASKBAR_PROGRESS_STYLE)
const selectedProgressPosition = shallowRef<TaskbarProgressPosition>(
  DEFAULT_TASKBAR_PROGRESS_POSITION,
)
const progressStyleSaving = shallowRef(false)
const progressPositionSaving = shallowRef(false)

/** 恢复已保存的播放进度样式。 */
async function loadProgressStyle() {
  try {
    ;[selectedProgressStyle.value, selectedProgressPosition.value] = await Promise.all([
      getTaskbarProgressStyle(),
      getTaskbarProgressPosition(),
    ])
  } catch (error) {
    console.error('读取播放进度样式失败', error)
  }
}

/** 保存横条进度位置，失败时恢复原值。 */
async function selectProgressPosition(value: unknown) {
  if (
    progressPositionSaving.value ||
    !isTaskbarProgressPosition(value) ||
    value === selectedProgressPosition.value
  ) {
    return
  }

  const previousPosition = selectedProgressPosition.value
  selectedProgressPosition.value = value
  progressPositionSaving.value = true
  try {
    await setTaskbarProgressPosition(value)
  } catch (error) {
    selectedProgressPosition.value = previousPosition
    notifySettingSaveFailed(t('settings.taskbar.progress.position'), error)
  } finally {
    progressPositionSaving.value = false
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
    notifySettingSaveFailed(t('settings.taskbar.progress.title'), error)
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
  <CollapsibleItem>
    <ItemMedia class="icon-tone-rose-500">
      <Activity />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.progress.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.progress.description') }}</ItemDescription>
    </ItemContent>
    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.progress.style') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.progress.styleDescription')
            }}</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedProgressStyle" @update:model-value="selectProgressStyle">
            <TabsList>
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
        </Field>

        <Field orientation="horizontal" :data-disabled="selectedProgressStyle !== 'bottom'">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.progress.position') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.progress.positionDescription')
            }}</FieldDescription>
          </FieldContent>
          <Tabs
            :model-value="selectedProgressPosition"
            @update:model-value="selectProgressPosition"
          >
            <TabsList>
              <TabsTrigger
                v-for="option in progressPositionOptions"
                :key="option.value"
                :value="option.value"
                :disabled="progressPositionSaving || selectedProgressStyle !== 'bottom'"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
