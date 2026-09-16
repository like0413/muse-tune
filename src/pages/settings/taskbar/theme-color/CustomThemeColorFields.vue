<script setup lang="ts">
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldTitle,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { TASKBAR_THEME_PRESET_COLORS } from '@/features/theme/colors'

const { t } = useI18n({ useScope: 'global' })

defineProps<{
  selectedColor: string
  draft: string
  error: string
  disabled: boolean
}>()

const emit = defineEmits<{
  selectColor: [color: string]
  updateDraft: [value: string | number]
  commit: []
}>()

/** 从原生颜色输入事件中读取颜色值。 */
function selectNativeColor(event: Event) {
  emit('selectColor', (event.target as HTMLInputElement).value)
}
</script>

<template>
  <Field orientation="horizontal">
    <FieldContent>
      <FieldTitle>{{ t('settings.taskbar.theme.presets') }}</FieldTitle>
      <FieldDescription>{{ t('settings.taskbar.theme.presetsDescription') }}</FieldDescription>
    </FieldContent>
    <div class="flex w-56 flex-nowrap justify-end gap-1">
      <button
        v-for="color in TASKBAR_THEME_PRESET_COLORS"
        :key="color"
        type="button"
        class="ring-offset-background size-4 shrink-0 rounded-full border transition-transform hover:scale-125 focus-visible:ring-2 focus-visible:ring-offset-1 focus-visible:outline-none"
        :class="selectedColor === color ? 'ring-ring ring-2 ring-offset-2' : ''"
        :style="{ backgroundColor: color }"
        :disabled="disabled"
        :aria-label="t('settings.taskbar.theme.useColor', { color })"
        @click="emit('selectColor', color)"
      />
    </div>
  </Field>

  <Field orientation="horizontal" :data-invalid="!!error">
    <FieldContent>
      <FieldTitle>{{ t('settings.taskbar.theme.customValue') }}</FieldTitle>
      <FieldDescription>{{ t('settings.taskbar.theme.customDescription') }}</FieldDescription>
      <FieldError v-if="error">{{ error }}</FieldError>
    </FieldContent>
    <div class="flex w-56 items-center gap-2">
      <input
        type="color"
        class="border-input h-9 w-11 cursor-pointer rounded-md border bg-transparent p-1 disabled:cursor-not-allowed disabled:opacity-50"
        :value="selectedColor"
        :disabled="disabled"
        :aria-label="t('settings.taskbar.theme.selectCustom')"
        @change="selectNativeColor"
      />
      <Input
        class="font-mono"
        :model-value="draft"
        :disabled="disabled"
        :aria-invalid="!!error"
        :aria-label="t('common.hexValue', { item: t('settings.taskbar.theme.title') })"
        @update:model-value="emit('updateDraft', $event)"
        @blur="emit('commit')"
        @keydown.enter="emit('commit')"
      />
    </div>
  </Field>
</template>
