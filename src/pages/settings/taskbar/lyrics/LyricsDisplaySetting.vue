<script setup lang="ts">
import { Captions } from '@lucide/vue'
import { onMounted, shallowRef } from 'vue'

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
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import {
  DEFAULT_TASKBAR_LYRICS_SETTINGS,
  getTaskbarLyricsSettings,
  isTaskbarLyricsAlignment,
  isTaskbarLyricsLineMode,
  normalizeTaskbarLyricsSettings,
  setTaskbarLyricsSettings,
  type TaskbarLyricsSettings,
} from '@/features/settings/lyrics'

const alignmentOptions = [
  { value: 'left', label: '左' },
  { value: 'center', label: '中' },
  { value: 'right', label: '右' },
] as const
const lineModeOptions = [
  { value: 'single', label: '单行' },
  { value: 'double', label: '双行' },
] as const

const selectedSettings = shallowRef<TaskbarLyricsSettings>({
  ...DEFAULT_TASKBAR_LYRICS_SETTINGS,
})
const committedSettings = shallowRef<TaskbarLyricsSettings>({
  ...DEFAULT_TASKBAR_LYRICS_SETTINGS,
})
const settingsSaving = shallowRef(false)

/** 恢复已保存的歌词显示配置。 */
async function loadSettings() {
  try {
    const settings = await getTaskbarLyricsSettings()
    selectedSettings.value = settings
    committedSettings.value = { ...settings }
  } catch (error) {
    console.error('读取歌词显示配置失败', error)
  }
}

/** 合并并保存一次配置变更，失败时恢复最近成功状态。 */
async function updateSettings(patch: Partial<TaskbarLyricsSettings>) {
  if (settingsSaving.value) return
  const next = normalizeTaskbarLyricsSettings({ ...selectedSettings.value, ...patch })
  selectedSettings.value = next
  settingsSaving.value = true
  try {
    await setTaskbarLyricsSettings(next)
    committedSettings.value = { ...next }
  } catch (error) {
    selectedSettings.value = { ...committedSettings.value }
    console.error('保存歌词显示配置失败', error)
  } finally {
    settingsSaving.value = false
  }
}

/** 接收单选组件的对齐值。 */
function selectAlignment(value: unknown) {
  if (isTaskbarLyricsAlignment(value)) void updateSettings({ alignment: value })
}

/** 接收单选组件的行数模式。 */
function selectLineMode(value: unknown) {
  if (isTaskbarLyricsLineMode(value)) void updateSettings({ lineMode: value })
}

onMounted(loadSettings)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-cyan-500">
      <Captions />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>歌词显示</ItemTitle>
      <ItemDescription>设置任务栏歌词的布局和逐字效果</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-lyrics-enabled">开启歌词</FieldLabel>
            <FieldDescription>关闭后恢复显示歌曲信息和控制按钮</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-lyrics-enabled"
            :model-value="selectedSettings.enabled"
            :disabled="settingsSaving"
            @update:model-value="updateSettings({ enabled: $event })"
          />
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.enabled">
          <FieldContent>
            <FieldTitle>对齐方式</FieldTitle>
            <FieldDescription>歌词在剩余空白区域内的水平位置</FieldDescription>
          </FieldContent>
          <ToggleGroup
            type="single"
            variant="outline"
            :model-value="selectedSettings.alignment"
            :disabled="settingsSaving || !selectedSettings.enabled"
            aria-label="歌词对齐方式"
            @update:model-value="selectAlignment"
          >
            <ToggleGroupItem
              v-for="option in alignmentOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </ToggleGroupItem>
          </ToggleGroup>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.enabled">
          <FieldContent>
            <FieldTitle>显示行数</FieldTitle>
            <FieldDescription>双行时优先在第二行显示翻译，否则显示下一句</FieldDescription>
          </FieldContent>
          <ToggleGroup
            type="single"
            variant="outline"
            :model-value="selectedSettings.lineMode"
            :disabled="settingsSaving || !selectedSettings.enabled"
            aria-label="歌词显示行数"
            @update:model-value="selectLineMode"
          >
            <ToggleGroupItem
              v-for="option in lineModeOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </ToggleGroupItem>
          </ToggleGroup>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.enabled">
          <FieldContent>
            <FieldLabel for="taskbar-lyrics-word-highlight">逐字高亮</FieldLabel>
            <FieldDescription>仅在歌词源包含逐字时间轴时生效，否则自动按整行显示</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-lyrics-word-highlight"
            :model-value="selectedSettings.wordHighlight"
            :disabled="settingsSaving || !selectedSettings.enabled"
            @update:model-value="updateSettings({ wordHighlight: $event })"
          />
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
