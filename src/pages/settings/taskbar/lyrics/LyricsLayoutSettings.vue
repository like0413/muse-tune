<script setup lang="ts">
import type { DeepReadonly } from 'vue'

import {
  Field,
  FieldContent,
  FieldDescription,
  FieldLabel,
  FieldTitle,
} from '@/components/ui/field'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  isTaskbarLyricsAlignment,
  isTaskbarLyricsLineMode,
  isTaskbarLyricsSecondaryLine,
  TASKBAR_LYRICS_FONT_SIZE_MAX,
  TASKBAR_LYRICS_FONT_SIZE_MIN,
  type TaskbarLyricsSettings,
} from '@/features/settings/lyrics'

import LyricsAppearanceSettings from './LyricsAppearanceSettings.vue'

defineProps<{
  settings: DeepReadonly<TaskbarLyricsSettings>
  saving: boolean
}>()

const emit = defineEmits<{
  updateSettings: [patch: Partial<TaskbarLyricsSettings>]
  previewFontSize: [values: number[] | undefined]
  commitFontSize: [values: number[] | undefined]
}>()

const alignmentOptions = [
  { value: 'left', label: '左' },
  { value: 'center', label: '中' },
  { value: 'right', label: '右' },
] as const
const lineModeOptions = [
  { value: 'single', label: '单行' },
  { value: 'double', label: '双行' },
] as const
const secondaryLineOptions = [
  { value: 'translation_only', label: '仅翻译' },
  { value: 'next', label: '下一句' },
  { value: 'translation_or_next', label: '翻译优先' },
] as const

/** 接收单选组件的对齐值。 */
function selectAlignment(value: unknown) {
  if (isTaskbarLyricsAlignment(value)) emit('updateSettings', { alignment: value })
}

/** 接收单选组件的行数模式。 */
function selectLineMode(value: unknown) {
  if (isTaskbarLyricsLineMode(value)) emit('updateSettings', { lineMode: value })
}

/** 接收双行次要内容的优先选择。 */
function selectSecondaryLine(value: unknown) {
  if (isTaskbarLyricsSecondaryLine(value)) emit('updateSettings', { secondaryLine: value })
}
</script>

<template>
  <Field orientation="horizontal">
    <FieldContent>
      <FieldLabel for="taskbar-lyrics-enabled">开启歌词</FieldLabel>
      <FieldDescription>关闭后恢复显示歌曲信息和控制按钮</FieldDescription>
    </FieldContent>
    <Switch
      id="taskbar-lyrics-enabled"
      :model-value="settings.enabled"
      :disabled="saving"
      @update:model-value="emit('updateSettings', { enabled: $event })"
    />
  </Field>

  <Field orientation="horizontal" :data-disabled="!settings.enabled">
    <FieldContent>
      <FieldTitle>显示行数</FieldTitle>
      <FieldDescription>双行时可指定第二行优先显示的内容</FieldDescription>
    </FieldContent>
    <Tabs :model-value="settings.lineMode" @update:model-value="selectLineMode">
      <TabsList aria-label="歌词显示行数">
        <TabsTrigger
          v-for="option in lineModeOptions"
          :key="option.value"
          :value="option.value"
          :disabled="saving || !settings.enabled"
        >
          {{ option.label }}
        </TabsTrigger>
      </TabsList>
    </Tabs>
  </Field>

  <Field
    orientation="horizontal"
    :data-disabled="!settings.enabled || settings.lineMode !== 'double'"
  >
    <FieldContent>
      <FieldTitle>第二行回退规则</FieldTitle>
      <FieldDescription>仅“翻译优先”会在没有翻译时显示下一句</FieldDescription>
    </FieldContent>
    <Tabs :model-value="settings.secondaryLine" @update:model-value="selectSecondaryLine">
      <TabsList aria-label="双行歌词第二行优先内容">
        <TabsTrigger
          v-for="option in secondaryLineOptions"
          :key="option.value"
          :value="option.value"
          :disabled="saving || !settings.enabled || settings.lineMode !== 'double'"
        >
          {{ option.label }}
        </TabsTrigger>
      </TabsList>
    </Tabs>
  </Field>

  <Field orientation="horizontal" :data-disabled="!settings.enabled">
    <FieldContent>
      <FieldLabel for="taskbar-lyrics-word-highlight">逐字高亮</FieldLabel>
      <FieldDescription>仅在歌词源包含逐字时间轴时生效</FieldDescription>
    </FieldContent>
    <Switch
      id="taskbar-lyrics-word-highlight"
      :model-value="settings.wordHighlight"
      :disabled="saving || !settings.enabled"
      @update:model-value="emit('updateSettings', { wordHighlight: $event })"
    />
  </Field>

  <Field orientation="horizontal" :data-disabled="!settings.enabled">
    <FieldContent>
      <FieldTitle>对齐方式</FieldTitle>
      <FieldDescription>歌词在空白区域内的水平位置</FieldDescription>
    </FieldContent>
    <Tabs :model-value="settings.alignment" @update:model-value="selectAlignment">
      <TabsList aria-label="歌词对齐方式">
        <TabsTrigger
          v-for="option in alignmentOptions"
          :key="option.value"
          :value="option.value"
          :disabled="saving || !settings.enabled"
        >
          {{ option.label }}
        </TabsTrigger>
      </TabsList>
    </Tabs>
  </Field>

  <Field orientation="horizontal" :data-disabled="!settings.enabled">
    <FieldContent>
      <FieldTitle>歌词大小</FieldTitle>
      <FieldDescription>字号范围为 10–18px，双行时自动收紧行距</FieldDescription>
    </FieldContent>
    <div class="flex w-56 items-center gap-3">
      <Slider
        :model-value="[settings.fontSize]"
        :min="TASKBAR_LYRICS_FONT_SIZE_MIN"
        :max="TASKBAR_LYRICS_FONT_SIZE_MAX"
        :step="1"
        :disabled="saving || !settings.enabled"
        aria-label="歌词大小"
        @update:model-value="emit('previewFontSize', $event)"
        @value-commit="emit('commitFontSize', $event)"
      />
      <output class="text-muted-foreground w-12 text-right text-xs tabular-nums">
        {{ settings.fontSize }}px
      </output>
    </div>
  </Field>

  <LyricsAppearanceSettings
    :settings="settings"
    :disabled="saving || !settings.enabled"
    @update-settings="emit('updateSettings', $event)"
  />
</template>
