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
import {
  DEFAULT_TASKBAR_AUTO_HIDE,
  getTaskbarAutoHide,
  setTaskbarAutoHide,
  type TaskbarAutoHide,
} from '@/features/settings/bar-visibility'

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
    console.error('保存任务栏播放器自动隐藏配置失败', error)
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
      <ItemTitle>自动隐藏</ItemTitle>
      <ItemDescription>根据媒体会话和播放状态自动隐藏 bar</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-hide-when-paused">暂停时隐藏</FieldLabel>
            <FieldDescription>当前播放器进入暂停状态时隐藏 bar</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-hide-when-paused"
            :model-value="selectedPreference.whenPaused"
            :disabled="saving"
            @update:model-value="updatePreference({ whenPaused: $event })"
          />
        </Field>

        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-hide-without-session">无媒体会话时隐藏</FieldLabel>
            <FieldDescription>Windows 没有可用媒体会话时隐藏 bar</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-hide-without-session"
            :model-value="selectedPreference.whenNoMediaSession"
            :disabled="saving"
            @update:model-value="updatePreference({ whenNoMediaSession: $event })"
          />
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
