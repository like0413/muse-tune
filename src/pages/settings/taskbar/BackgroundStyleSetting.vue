<script setup lang="ts">
import { Wallpaper } from '@lucide/vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import { useEventState } from '@/features/ipc/useEventState'
import {
  DEFAULT_TASKBAR_BACKGROUND_STYLE,
  getTaskbarBackgroundStyle,
  isTaskbarBackgroundStyle,
  listenTaskbarBackgroundStyleChange,
  setTaskbarBackgroundStyle,
} from '@/features/settings/background-style'

import CompatibilityNoticeDialog from './components/CompatibilityNoticeDialog.vue'

const { t } = useI18n({ useScope: 'global' })
/** 当前背景样式；订阅变更以接收另一项设置触发的兼容模式切换。 */
const selectedStyle = useEventState(
  {
    read: getTaskbarBackgroundStyle,
    subscribe: listenTaskbarBackgroundStyleChange,
    failureMessage: '读取背景样式失败',
  },
  DEFAULT_TASKBAR_BACKGROUND_STYLE,
)
const saving = shallowRef(false)
const showCompatibilityNotice = shallowRef(false)

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
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-violet-500"><Wallpaper /></ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.backgroundStyle.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.backgroundStyle.description') }}</ItemDescription>
    </ItemContent>
    <ItemActions>
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
    </ItemActions>
  </Item>
  <CompatibilityNoticeDialog
    v-model:open="showCompatibilityNotice"
    :description="t('settings.taskbar.backgroundStyle.progressFallback')"
  />
</template>
