<script setup lang="ts">
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldTitle,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { TASKBAR_THEME_PRESET_COLORS } from '@/features/settings/theme-color'

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
      <FieldTitle>候选颜色</FieldTitle>
      <FieldDescription>选择常用颜色，点击后立即应用</FieldDescription>
    </FieldContent>
    <div class="flex w-56 flex-nowrap justify-end gap-1" aria-label="候选主题色">
      <button
        v-for="color in TASKBAR_THEME_PRESET_COLORS"
        :key="color"
        type="button"
        class="ring-offset-background size-4 shrink-0 rounded-full border transition-transform hover:scale-125 focus-visible:ring-2 focus-visible:ring-offset-1 focus-visible:outline-none"
        :class="selectedColor === color ? 'ring-ring ring-2 ring-offset-2' : ''"
        :style="{ backgroundColor: color }"
        :disabled="disabled"
        :aria-label="`使用颜色 ${color}`"
        @click="emit('selectColor', color)"
      />
    </div>
  </Field>

  <Field orientation="horizontal" :data-invalid="!!error">
    <FieldContent>
      <FieldTitle>自定义颜色值</FieldTitle>
      <FieldDescription>支持 #RRGGBB 格式</FieldDescription>
      <FieldError v-if="error">{{ error }}</FieldError>
    </FieldContent>
    <div class="flex w-56 items-center gap-2">
      <input
        type="color"
        class="border-input h-9 w-11 cursor-pointer rounded-md border bg-transparent p-1 disabled:cursor-not-allowed disabled:opacity-50"
        :value="selectedColor"
        :disabled="disabled"
        aria-label="选择自定义主题色"
        @change="selectNativeColor"
      />
      <Input
        class="font-mono"
        :model-value="draft"
        :disabled="disabled"
        :aria-invalid="!!error"
        aria-label="自定义主题色十六进制值"
        @update:model-value="emit('updateDraft', $event)"
        @blur="emit('commit')"
        @keydown.enter="emit('commit')"
      />
    </div>
  </Field>
</template>
