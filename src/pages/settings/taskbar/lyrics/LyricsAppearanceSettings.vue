<script setup lang="ts">
import type { DeepReadonly } from 'vue'
import { reactive, watch } from 'vue'

import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldTitle,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  isTaskbarLyricsColorScheme,
  type TaskbarLyricsColorScheme,
  type TaskbarLyricsSettings,
} from '@/features/settings/lyrics'
import { normalizeHexColor } from '@/features/settings/theme-color'

import LyricsFontPicker from './LyricsFontPicker.vue'

const props = defineProps<{
  settings: DeepReadonly<TaskbarLyricsSettings>
  disabled: boolean
}>()

const emit = defineEmits<{
  updateSettings: [patch: Partial<TaskbarLyricsSettings>]
}>()
const { t } = useI18n({ useScope: 'global' })

type EditableColor = 'playedColor' | 'unplayedColor'

const colorSchemeOptions = computed(
  () =>
    [
      { value: 'theme', label: t('settings.taskbar.lyrics.followTheme') },
      { value: 'custom', label: t('common.custom') },
    ] as const satisfies ReadonlyArray<{ value: TaskbarLyricsColorScheme; label: string }>,
)

const colorFields = computed(
  () =>
    [
      {
        key: 'playedColor',
        title: t('settings.taskbar.lyrics.playedColor'),
        description: t('settings.taskbar.lyrics.playedColorDescription'),
      },
      {
        key: 'unplayedColor',
        title: t('settings.taskbar.lyrics.unplayedColor'),
        description: t('settings.taskbar.lyrics.unplayedColorDescription'),
      },
    ] as const,
)

const colorDrafts = reactive<Record<EditableColor, string>>({
  playedColor: props.settings.playedColor,
  unplayedColor: props.settings.unplayedColor,
})
const colorErrors = reactive<Record<EditableColor, string>>({
  playedColor: '',
  unplayedColor: '',
})

/** 保存成功或失败回滚后，同步父组件中的权威颜色值。 */
watch(
  () => [props.settings.playedColor, props.settings.unplayedColor] as const,
  ([playedColor, unplayedColor]) => {
    colorDrafts.playedColor = playedColor
    colorDrafts.unplayedColor = unplayedColor
    colorErrors.playedColor = ''
    colorErrors.unplayedColor = ''
  },
)

/** 接收标签页返回的歌词配色模式。 */
function selectColorScheme(value: unknown) {
  if (isTaskbarLyricsColorScheme(value)) emit('updateSettings', { colorScheme: value })
}

/** 更新十六进制颜色草稿，输入期间不写入设置文件。 */
function updateColorDraft(key: EditableColor, value: string | number) {
  colorDrafts[key] = String(value)
  colorErrors[key] = ''
}

/** 校验并提交一个自定义颜色，同时切换到自定义方案。 */
function commitColor(key: EditableColor) {
  const color = normalizeHexColor(colorDrafts[key])
  if (!color) {
    colorErrors[key] = String(t('settings.taskbar.lyrics.invalidColor'))
    return
  }
  colorDrafts[key] = color
  colorErrors[key] = ''
  emit('updateSettings', { colorScheme: 'custom', [key]: color })
}

/** 原生颜色选择器只会产生有效颜色，可直接提交。 */
function selectNativeColor(key: EditableColor, event: Event) {
  colorDrafts[key] = (event.target as HTMLInputElement).value
  commitColor(key)
}
</script>

<template>
  <Field orientation="horizontal" :data-disabled="disabled">
    <FieldContent>
      <FieldTitle>{{ t('settings.taskbar.lyrics.colors') }}</FieldTitle>
      <FieldDescription>{{ t('settings.taskbar.lyrics.colorsDescription') }}</FieldDescription>
    </FieldContent>
    <Tabs :model-value="settings.colorScheme" @update:model-value="selectColorScheme">
      <TabsList>
        <TabsTrigger
          v-for="option in colorSchemeOptions"
          :key="option.value"
          :value="option.value"
          :disabled="disabled"
        >
          {{ option.label }}
        </TabsTrigger>
      </TabsList>
    </Tabs>
  </Field>

  <template v-if="settings.colorScheme === 'custom'">
    <Field
      v-for="field in colorFields"
      :key="field.key"
      orientation="horizontal"
      :data-disabled="disabled"
      :data-invalid="!!colorErrors[field.key]"
    >
      <FieldContent>
        <FieldTitle>{{ field.title }}</FieldTitle>
        <FieldDescription>{{ field.description }}</FieldDescription>
        <FieldError v-if="colorErrors[field.key]">{{ colorErrors[field.key] }}</FieldError>
      </FieldContent>
      <div class="flex w-56 items-center gap-2">
        <input
          type="color"
          class="border-input h-9 w-11 cursor-pointer rounded-md border bg-transparent p-1 disabled:cursor-not-allowed disabled:opacity-50"
          :value="settings[field.key]"
          :disabled="disabled"
          :aria-label="field.title"
          @change="selectNativeColor(field.key, $event)"
        />
        <Input
          class="font-mono"
          :model-value="colorDrafts[field.key]"
          :disabled="disabled"
          :aria-invalid="!!colorErrors[field.key]"
          :aria-label="t('common.hexValue', { item: field.title })"
          @update:model-value="updateColorDraft(field.key, $event)"
          @blur="commitColor(field.key)"
          @keydown.enter="commitColor(field.key)"
        />
      </div>
    </Field>
  </template>

  <Field orientation="horizontal" :data-disabled="disabled">
    <FieldContent>
      <FieldTitle>{{ t('settings.taskbar.lyrics.font') }}</FieldTitle>
      <FieldDescription>{{ t('settings.taskbar.lyrics.fontDescription') }}</FieldDescription>
    </FieldContent>
    <LyricsFontPicker
      :model-value="settings.fontFamily"
      :disabled="disabled"
      @update:model-value="emit('updateSettings', { fontFamily: $event })"
    />
  </Field>
</template>
