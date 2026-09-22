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
  type TaskbarBackgroundStyle,
} from '@/features/settings/background-style'
import { getTaskbarProgressStyle } from '@/features/settings/progress-style'

import CompatibilityConfirmDialog from './components/CompatibilityConfirmDialog.vue'

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
const pendingStyle = shallowRef<TaskbarBackgroundStyle | null>(null)
const compatibilityDialogOpen = computed({
  get: () => pendingStyle.value !== null,
  set: (open: boolean) => {
    if (!open) pendingStyle.value = null
  },
})

/** 保存已经确认的背景样式；未确认值不会进入持久化或事件链路。 */
async function saveStyle(value: TaskbarBackgroundStyle) {
  saving.value = true
  try {
    await setTaskbarBackgroundStyle(value)
    selectedStyle.value = value
  } catch (error) {
    notifySettingSaveFailed(t('settings.taskbar.backgroundStyle.title'), error)
  } finally {
    saving.value = false
  }
}

/** 冲突选择只打开确认弹窗，当前设置与任务栏显示保持不变。 */
async function selectStyle(value: unknown) {
  if (saving.value || !isTaskbarBackgroundStyle(value) || value === selectedStyle.value) return
  if (value === 'theme') {
    await saveStyle(value)
    return
  }

  saving.value = true
  try {
    const progressStyle = await getTaskbarProgressStyle()
    if (progressStyle === 'vertical-gradient') {
      pendingStyle.value = value
      return
    }
  } catch (error) {
    notifySettingSaveFailed(t('settings.taskbar.backgroundStyle.title'), error)
    return
  } finally {
    saving.value = false
  }
  await saveStyle(value)
}

/** 仅由弹窗确认按钮调用，应用并清空待确认背景样式。 */
function confirmPendingStyle() {
  const style = pendingStyle.value
  if (!style) return
  pendingStyle.value = null
  void saveStyle(style)
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
  <CompatibilityConfirmDialog
    v-model:open="compatibilityDialogOpen"
    :description="t('settings.taskbar.backgroundStyle.progressFallback')"
    @confirm="confirmPendingStyle"
  />
</template>
