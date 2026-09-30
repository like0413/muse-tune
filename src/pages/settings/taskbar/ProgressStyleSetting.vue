<script setup lang="ts">
import { Gauge } from '@lucide/vue'

import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldTitle,
} from '@/components/ui/field'
import { ItemContent, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifySettingSaveFailed, reportBackgroundFailure } from '@/features/feedback/errors'
import { useEventState } from '@/features/ipc/useEventState'
import {
  getTaskbarBackgroundStyle,
  setCompatibleTaskbarProgressStyle,
} from '@/features/settings/background-style'
import {
  DEFAULT_TASKBAR_PROGRESS_POSITION,
  DEFAULT_TASKBAR_PROGRESS_STYLE,
  DEFAULT_TASKBAR_PROGRESS_VISIBLE,
  getTaskbarProgressPosition,
  getTaskbarProgressStyle,
  getTaskbarProgressVisible,
  isTaskbarProgressPosition,
  isTaskbarProgressStyle,
  listenTaskbarProgressStyleChange,
  setTaskbarProgressPosition,
  setTaskbarProgressVisible,
  type TaskbarProgressPosition,
  type TaskbarProgressStyle,
} from '@/features/settings/progress-style'

import CompatibilityConfirmDialog from './components/CompatibilityConfirmDialog.vue'

const { t } = useI18n({ useScope: 'global' })

const progressStyleOptions = computed(
  () =>
    [
      { value: 'bottom', label: t('settings.taskbar.progress.bar') },
      { value: 'cover-ring', label: t('settings.taskbar.progress.coverRing') },
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

const selectedProgressStyle = useEventState(
  {
    read: getTaskbarProgressStyle,
    subscribe: listenTaskbarProgressStyleChange,
    failureMessage: '读取播放进度样式失败',
  },
  DEFAULT_TASKBAR_PROGRESS_STYLE,
)
const selectedProgressVisible = shallowRef(DEFAULT_TASKBAR_PROGRESS_VISIBLE)
const selectedProgressPosition = shallowRef<TaskbarProgressPosition>(
  DEFAULT_TASKBAR_PROGRESS_POSITION,
)
const progressStyleSaving = shallowRef(false)
const progressPositionSaving = shallowRef(false)
const progressVisibilitySaving = shallowRef(false)
const pendingProgressStyle = shallowRef<TaskbarProgressStyle | null>(null)
const compatibilityDialogOpen = computed({
  get: () => pendingProgressStyle.value !== null,
  set: (open: boolean) => {
    if (!open) pendingProgressStyle.value = null
  },
})

/** 恢复进度条显隐与位置；这两项没有实时事件，只在进入设置页时读取一次。 */
async function loadProgressVisibilityAndPosition() {
  try {
    const [savedPosition, savedVisible] = await Promise.all([
      getTaskbarProgressPosition(),
      getTaskbarProgressVisible(),
    ])
    selectedProgressPosition.value = savedPosition
    selectedProgressVisible.value = savedVisible
  } catch (error) {
    reportBackgroundFailure('读取播放进度显隐与位置失败', error)
  }
}

/** 保存进度条显隐，失败时恢复此前状态。 */
async function selectProgressVisible(visible: boolean) {
  if (progressVisibilitySaving.value || visible === selectedProgressVisible.value) return
  const previous = selectedProgressVisible.value
  selectedProgressVisible.value = visible
  progressVisibilitySaving.value = true
  try {
    await setTaskbarProgressVisible(visible)
  } catch (error) {
    selectedProgressVisible.value = previous
    notifySettingSaveFailed(t('settings.taskbar.progress.title'), error)
  } finally {
    progressVisibilitySaving.value = false
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

/** 保存已经确认的播放进度样式，并在失败时恢复持久化值与任务栏显示。 */
async function saveProgressStyle(value: TaskbarProgressStyle) {
  const previousStyle = selectedProgressStyle.value
  progressStyleSaving.value = true

  try {
    await setCompatibleTaskbarProgressStyle(value)
    selectedProgressStyle.value = value
  } catch (error) {
    notifySettingSaveFailed(t('settings.taskbar.progress.title'), error)
    try {
      await setCompatibleTaskbarProgressStyle(previousStyle)
    } catch (rollbackError) {
      reportBackgroundFailure('恢复之前的播放进度样式失败', rollbackError)
    }
  } finally {
    progressStyleSaving.value = false
  }
}

/** 冲突选择只打开确认弹窗，当前设置与任务栏显示保持不变。 */
async function selectProgressStyle(value: unknown) {
  if (
    progressStyleSaving.value ||
    !isTaskbarProgressStyle(value) ||
    value === selectedProgressStyle.value
  ) {
    return
  }
  if (value !== 'vertical-gradient') {
    await saveProgressStyle(value)
    return
  }

  progressStyleSaving.value = true
  try {
    const backgroundStyle = await getTaskbarBackgroundStyle()
    if (backgroundStyle !== 'theme') {
      pendingProgressStyle.value = value
      return
    }
  } catch (error) {
    notifySettingSaveFailed(t('settings.taskbar.progress.title'), error)
    return
  } finally {
    progressStyleSaving.value = false
  }
  await saveProgressStyle(value)
}

/** 仅由弹窗确认按钮调用，应用并清空待确认进度样式。 */
function confirmPendingProgressStyle() {
  const style = pendingProgressStyle.value
  if (!style) return
  pendingProgressStyle.value = null
  void saveProgressStyle(style)
}

onMounted(() => void loadProgressVisibilityAndPosition())
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-rose-500">
      <Gauge />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.progress.title') }}</ItemTitle>
    </ItemContent>
    <template #actions>
      <Switch
        :model-value="selectedProgressVisible"
        :disabled="progressVisibilitySaving"
        :aria-label="t('settings.taskbar.progress.visible')"
        @update:model-value="selectProgressVisible"
      />
    </template>
    <template #content>
      <FieldGroup>
        <Field orientation="horizontal" :data-disabled="!selectedProgressVisible">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.progress.style') }}</FieldTitle>
          </FieldContent>
          <Tabs :model-value="selectedProgressStyle" @update:model-value="selectProgressStyle">
            <TabsList>
              <TabsTrigger
                v-for="option in progressStyleOptions"
                :key="option.value"
                :value="option.value"
                :disabled="progressStyleSaving || !selectedProgressVisible"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>

        <Field
          orientation="horizontal"
          :data-disabled="!selectedProgressVisible || selectedProgressStyle !== 'bottom'"
        >
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
                :disabled="
                  progressPositionSaving ||
                  !selectedProgressVisible ||
                  selectedProgressStyle !== 'bottom'
                "
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
  <CompatibilityConfirmDialog
    v-model:open="compatibilityDialogOpen"
    :description="t('settings.taskbar.backgroundStyle.backgroundFallback')"
    @confirm="confirmPendingProgressStyle"
  />
</template>
