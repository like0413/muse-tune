<script setup lang="ts">
import type { DeepReadonly } from 'vue'

import { Field, FieldContent, FieldDescription, FieldTitle } from '@/components/ui/field'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import {
  isTaskbarLyricsNetworkPolicy,
  isTaskbarLyricsOnlineStrategy,
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

const networkPolicyOptions = computed(
  () =>
    [
      { value: 'auto', label: t('common.auto') },
      { value: 'local_only', label: t('settings.taskbar.lyrics.localOnly') },
    ] as const,
)
const onlineStrategyOptions = computed(
  () =>
    [
      { value: 'parallel', label: t('settings.taskbar.lyrics.parallel') },
      { value: 'current_player_first', label: t('settings.taskbar.lyrics.currentFirst') },
    ] as const,
)

/** 接收联网策略选项。 */
function selectNetworkPolicy(value: unknown) {
  if (isTaskbarLyricsNetworkPolicy(value)) emit('updateSettings', { networkPolicy: value })
}

/** 接收在线歌词调度策略。 */
function selectOnlineStrategy(value: unknown) {
  if (isTaskbarLyricsOnlineStrategy(value)) emit('updateSettings', { onlineStrategy: value })
}
</script>

<template>
  <Field orientation="horizontal" :data-disabled="!settings.enabled">
    <FieldContent>
      <FieldTitle>{{ t('settings.taskbar.lyrics.network') }}</FieldTitle>
      <FieldDescription>{{ t('settings.taskbar.lyrics.networkDescription') }}</FieldDescription>
    </FieldContent>
    <Tabs :model-value="settings.networkPolicy" @update:model-value="selectNetworkPolicy">
      <TabsList>
        <TabsTrigger
          v-for="option in networkPolicyOptions"
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
    :data-disabled="!settings.enabled || settings.networkPolicy === 'local_only'"
  >
    <FieldContent>
      <FieldTitle>{{ t('settings.taskbar.lyrics.onlineStrategy') }}</FieldTitle>
      <FieldDescription>
        <div>{{ t('settings.taskbar.lyrics.parallelDescription') }}</div>
        <div>{{ t('settings.taskbar.lyrics.currentFirstDescription') }}</div>
      </FieldDescription>
    </FieldContent>
    <ToggleGroup
      type="single"
      variant="outline"
      :model-value="settings.onlineStrategy"
      :disabled="saving || !settings.enabled || settings.networkPolicy === 'local_only'"
      @update:model-value="selectOnlineStrategy"
    >
      <ToggleGroupItem
        v-for="option in onlineStrategyOptions"
        :key="option.value"
        :value="option.value"
      >
        {{ option.label }}
      </ToggleGroupItem>
    </ToggleGroup>
  </Field>
</template>
