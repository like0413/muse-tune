<script setup lang="ts">
import type { DeepReadonly } from 'vue'

import { Field, FieldContent, FieldDescription, FieldTitle } from '@/components/ui/field'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  isTaskbarLyricsChineseVariant,
  type TaskbarLyricsSettings,
} from '@/features/settings/lyrics'

const { t } = useI18n({ useScope: 'global' })

defineProps<{
  settings: DeepReadonly<TaskbarLyricsSettings>
  saving: boolean
}>()

const emit = defineEmits<{
  updateSettings: [patch: Partial<TaskbarLyricsSettings>]
}>()

const variantOptions = computed(
  () =>
    [
      { value: 'follow_interface', label: t('settings.taskbar.lyrics.followInterface') },
      { value: 'simplified', label: t('settings.taskbar.lyrics.simplified') },
      { value: 'traditional', label: t('settings.taskbar.lyrics.traditional') },
    ] as const,
)

/** 接收中文字形选项并提交给设置控制器。 */
function selectChineseVariant(value: unknown) {
  if (isTaskbarLyricsChineseVariant(value)) {
    emit('updateSettings', { chineseVariant: value })
  }
}
</script>

<template>
  <Field orientation="horizontal" :data-disabled="!settings.enabled">
    <FieldContent>
      <FieldTitle>{{ t('settings.taskbar.lyrics.chineseVariant') }}</FieldTitle>
      <FieldDescription>{{
        t('settings.taskbar.lyrics.chineseVariantDescription')
      }}</FieldDescription>
    </FieldContent>
    <Tabs :model-value="settings.chineseVariant" @update:model-value="selectChineseVariant">
      <TabsList>
        <TabsTrigger
          v-for="option in variantOptions"
          :key="option.value"
          :value="option.value"
          :disabled="saving || !settings.enabled"
        >
          {{ option.label }}
        </TabsTrigger>
      </TabsList>
    </Tabs>
  </Field>
</template>
