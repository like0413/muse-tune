<script setup lang="ts">
import { Images } from '@lucide/vue'
import type { UnlistenFn } from '@tauri-apps/api/event'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldTitle,
} from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_BACKGROUND_STYLE,
  DEFAULT_TASKBAR_BACKGROUND_FLOW,
  getTaskbarBackgroundFlow,
  getTaskbarBackgroundStyle,
  isTaskbarBackgroundStyle,
  listenTaskbarBackgroundFlowChange,
  listenTaskbarBackgroundStyleChange,
  setTaskbarBackgroundFlow,
  setTaskbarBackgroundStyle,
  type TaskbarBackgroundStyle,
} from '@/features/settings/background-style'

import CompatibilityNoticeDialog from './components/CompatibilityNoticeDialog.vue'

const { t } = useI18n({ useScope: 'global' })
const selectedStyle = shallowRef<TaskbarBackgroundStyle>(DEFAULT_TASKBAR_BACKGROUND_STYLE)
const flowEnabled = shallowRef(DEFAULT_TASKBAR_BACKGROUND_FLOW)
const saving = shallowRef(false)
const flowSaving = shallowRef(false)
const showCompatibilityNotice = shallowRef(false)
let unlistenStyle: UnlistenFn | undefined
let unlistenFlow: UnlistenFn | undefined
let styleRevision = 0
let flowRevision = 0
let disposed = false

/** 恢复设置并监听另一项设置触发的兼容模式切换。 */
async function initialize() {
  try {
    const [stopStyle, stopFlow] = await Promise.all([
      listenTaskbarBackgroundStyleChange((style) => {
        styleRevision += 1
        selectedStyle.value = style
      }),
      listenTaskbarBackgroundFlowChange((enabled) => {
        flowRevision += 1
        flowEnabled.value = enabled
      }),
    ])
    if (disposed) {
      stopStyle()
      stopFlow()
      return
    }
    unlistenStyle = stopStyle
    unlistenFlow = stopFlow
    const revision = styleRevision
    const savedFlowRevision = flowRevision
    const [savedStyle, savedFlow] = await Promise.all([
      getTaskbarBackgroundStyle(),
      getTaskbarBackgroundFlow(),
    ])
    if (!disposed && revision === styleRevision) selectedStyle.value = savedStyle
    if (!disposed && savedFlowRevision === flowRevision) flowEnabled.value = savedFlow
  } catch (error) {
    console.error('读取背景样式失败', error)
  }
}

/** 保存背景样式；发生兼容切换后提示用户。 */
async function selectStyle(value: unknown) {
  if (saving.value || !isTaskbarBackgroundStyle(value) || value === selectedStyle.value) return
  const previousStyle = selectedStyle.value
  saving.value = true
  try {
    const replacedProgress = await setTaskbarBackgroundStyle(value)
    selectedStyle.value = value
    if (replacedProgress) showCompatibilityNotice.value = true
  } catch (error) {
    selectedStyle.value = previousStyle
    notifySettingSaveFailed(t('settings.taskbar.backgroundStyle.title'), error)
  } finally {
    saving.value = false
  }
}

/** 独立保存流动偏好，关闭模糊封面不会重置该选择。 */
async function selectFlow(enabled: boolean) {
  if (flowSaving.value || enabled === flowEnabled.value) return
  const previous = flowEnabled.value
  flowEnabled.value = enabled
  flowSaving.value = true
  try {
    await setTaskbarBackgroundFlow(enabled)
  } catch (error) {
    flowEnabled.value = previous
    notifySettingSaveFailed(t('settings.taskbar.backgroundStyle.coverFlow'), error)
  } finally {
    flowSaving.value = false
  }
}

onMounted(initialize)
onUnmounted(() => {
  disposed = true
  unlistenStyle?.()
  unlistenFlow?.()
})
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-violet-500"><Images /></ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.backgroundStyle.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.backgroundStyle.description') }}</ItemDescription>
    </ItemContent>
    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.backgroundStyle.style') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.backgroundStyle.styleDescription')
            }}</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedStyle" @update:model-value="selectStyle">
            <TabsList>
              <TabsTrigger value="theme" :disabled="saving">{{
                t('settings.taskbar.backgroundStyle.theme')
              }}</TabsTrigger>
              <TabsTrigger value="cover-blur" :disabled="saving">{{
                t('settings.taskbar.backgroundStyle.coverBlur')
              }}</TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>
        <Field v-if="selectedStyle === 'cover-blur'" orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-background-flow">{{
              t('settings.taskbar.backgroundStyle.coverFlow')
            }}</FieldLabel>
            <FieldDescription>{{
              t('settings.taskbar.backgroundStyle.flowDescription')
            }}</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-background-flow"
            :model-value="flowEnabled"
            :disabled="flowSaving"
            @update:model-value="selectFlow"
          />
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
  <CompatibilityNoticeDialog
    v-model:open="showCompatibilityNotice"
    :description="t('settings.taskbar.backgroundStyle.progressFallback')"
  />
</template>
