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
  getTaskbarBackgroundStyle,
  isTaskbarBackgroundStyle,
  listenTaskbarBackgroundStyleChange,
  setTaskbarBackgroundStyle,
  type TaskbarBackgroundStyle,
} from '@/features/settings/background-style'

import CompatibilityNoticeDialog from './components/CompatibilityNoticeDialog.vue'

const { t } = useI18n({ useScope: 'global' })
const selectedStyle = shallowRef<TaskbarBackgroundStyle>(DEFAULT_TASKBAR_BACKGROUND_STYLE)
const saving = shallowRef(false)
const showCompatibilityNotice = shallowRef(false)
const baseStyle = computed(() => (selectedStyle.value === 'theme' ? 'theme' : 'cover-blur'))
let unlisten: UnlistenFn | undefined
let styleRevision = 0
let disposed = false

/** 恢复设置并监听另一项设置触发的兼容模式切换。 */
async function initialize() {
  try {
    const stop = await listenTaskbarBackgroundStyleChange((style) => {
      styleRevision += 1
      selectedStyle.value = style
    })
    if (disposed) return stop()
    unlisten = stop
    const revision = styleRevision
    const saved = await getTaskbarBackgroundStyle()
    if (!disposed && revision === styleRevision) selectedStyle.value = saved
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

/** 流动是模糊封面的附加效果，关闭后仍保留模糊封面。 */
function selectFlow(enabled: boolean) {
  if (selectedStyle.value === 'theme') return
  void selectStyle(enabled ? 'cover-flow' : 'cover-blur')
}

onMounted(initialize)
onUnmounted(() => {
  disposed = true
  unlisten?.()
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
          <Tabs :model-value="baseStyle" @update:model-value="selectStyle">
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
        <Field v-if="baseStyle === 'cover-blur'" orientation="horizontal">
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
            :model-value="selectedStyle === 'cover-flow'"
            :disabled="saving"
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
