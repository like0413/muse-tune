<script setup lang="ts">
import { EyeOff } from '@lucide/vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Switch } from '@/components/ui/switch'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_AUTO_HIDE,
  getTaskbarAutoHide,
  setTaskbarAutoHide,
  type TaskbarAutoHide,
} from '@/features/settings/bar-visibility'

const { t } = useI18n({ useScope: 'global' })

const selectedPreference = shallowRef<TaskbarAutoHide>({ ...DEFAULT_TASKBAR_AUTO_HIDE })
const committedPreference = shallowRef<TaskbarAutoHide>({ ...DEFAULT_TASKBAR_AUTO_HIDE })
const saving = shallowRef(false)

/** 恢复已保存的 bar 自动隐藏配置。 */
async function loadPreference() {
  try {
    const preference = await getTaskbarAutoHide()
    selectedPreference.value = preference
    committedPreference.value = { ...preference }
  } catch (error) {
    console.error('读取任务栏播放器自动隐藏配置失败', error)
  }
}

/** 合并并保存一次自动隐藏配置，失败时恢复已提交值。 */
async function updatePreference(patch: Partial<TaskbarAutoHide>) {
  if (saving.value) return
  const nextPreference = { ...selectedPreference.value, ...patch }
  selectedPreference.value = nextPreference
  saving.value = true
  try {
    await setTaskbarAutoHide(nextPreference)
    committedPreference.value = { ...nextPreference }
  } catch (error) {
    selectedPreference.value = { ...committedPreference.value }
    notifySettingSaveFailed(t('settings.taskbar.autoHide.title'), error)
  } finally {
    saving.value = false
  }
}

onMounted(loadPreference)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-slate-500">
      <EyeOff />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.autoHide.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.autoHide.description') }}</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-hide-without-session">{{
              t('settings.taskbar.autoHide.noSession')
            }}</FieldLabel>
            <FieldDescription>{{
              t('settings.taskbar.autoHide.noSessionDescription')
            }}</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-hide-without-session"
            :model-value="selectedPreference.whenNoMediaSession"
            :disabled="saving"
            @update:model-value="updatePreference({ whenNoMediaSession: $event })"
          />
        </Field>

        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-hide-when-paused">{{
              t('settings.taskbar.autoHide.paused')
            }}</FieldLabel>
            <FieldDescription>{{
              t('settings.taskbar.autoHide.pausedDescription')
            }}</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-hide-when-paused"
            :model-value="selectedPreference.whenPaused"
            :disabled="saving"
            @update:model-value="updatePreference({ whenPaused: $event })"
          />
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
