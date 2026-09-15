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

const { t } = useI18n({ useScope: 'global' })

defineProps<{
  settings: DeepReadonly<TaskbarLyricsSettings>
  saving: boolean
}>()

const emit = defineEmits<{
  updateSettings: [patch: Partial<TaskbarLyricsSettings>]
  previewFontSize: [values: number[] | undefined]
  commitFontSize: [values: number[] | undefined]
}>()

const alignmentOptions = computed(
  () =>
    [
      { value: 'left', label: t('common.left') },
      { value: 'center', label: t('common.center') },
      { value: 'right', label: t('common.right') },
    ] as const,
)
const lineModeOptions = computed(
  () =>
    [
      { value: 'single', label: t('settings.taskbar.lyrics.single') },
      { value: 'double', label: t('settings.taskbar.lyrics.double') },
    ] as const,
)
const secondaryLineOptions = computed(
  () =>
    [
      { value: 'translation_only', label: t('settings.taskbar.lyrics.translationOnly') },
      { value: 'next', label: t('settings.taskbar.lyrics.nextLine') },
      { value: 'translation_or_next', label: t('settings.taskbar.lyrics.translationFirst') },
    ] as const,
)

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
      <FieldLabel for="taskbar-lyrics-enabled">{{
        t('settings.taskbar.lyrics.enabled')
      }}</FieldLabel>
      <FieldDescription>{{ t('settings.taskbar.lyrics.enabledDescription') }}</FieldDescription>
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
      <FieldTitle>{{ t('settings.taskbar.lyrics.lines') }}</FieldTitle>
      <FieldDescription>{{ t('settings.taskbar.lyrics.linesDescription') }}</FieldDescription>
    </FieldContent>
    <Tabs :model-value="settings.lineMode" @update:model-value="selectLineMode">
      <TabsList>
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
      <FieldTitle>{{ t('settings.taskbar.lyrics.secondary') }}</FieldTitle>
      <FieldDescription>{{ t('settings.taskbar.lyrics.secondaryDescription') }}</FieldDescription>
    </FieldContent>
    <Tabs :model-value="settings.secondaryLine" @update:model-value="selectSecondaryLine">
      <TabsList>
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
      <FieldLabel for="taskbar-lyrics-word-highlight">{{
        t('settings.taskbar.lyrics.wordHighlight')
      }}</FieldLabel>
      <FieldDescription>{{
        t('settings.taskbar.lyrics.wordHighlightDescription')
      }}</FieldDescription>
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
      <FieldTitle>{{ t('common.alignment') }}</FieldTitle>
      <FieldDescription>{{ t('settings.taskbar.lyrics.alignmentDescription') }}</FieldDescription>
    </FieldContent>
    <Tabs :model-value="settings.alignment" @update:model-value="selectAlignment">
      <TabsList>
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
      <FieldTitle>{{ t('settings.taskbar.lyrics.size') }}</FieldTitle>
      <FieldDescription>{{ t('settings.taskbar.lyrics.sizeDescription') }}</FieldDescription>
    </FieldContent>
    <div class="flex w-56 items-center gap-3">
      <Slider
        :model-value="[settings.fontSize]"
        :min="TASKBAR_LYRICS_FONT_SIZE_MIN"
        :max="TASKBAR_LYRICS_FONT_SIZE_MAX"
        :step="1"
        :disabled="saving || !settings.enabled"
        :aria-label="t('settings.taskbar.lyrics.size')"
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
