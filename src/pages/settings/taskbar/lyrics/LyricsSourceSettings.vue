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

defineProps<{
  settings: DeepReadonly<TaskbarLyricsSettings>
  saving: boolean
}>()

const emit = defineEmits<{
  updateSettings: [patch: Partial<TaskbarLyricsSettings>]
}>()

const networkPolicyOptions = [
  { value: 'auto', label: '自动' },
  { value: 'local_only', label: '仅本地与缓存' },
] as const
const onlineStrategyOptions = [
  { value: 'parallel', label: '并行查询' },
  { value: 'current_player_first', label: '当前平台优先' },
] as const

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
      <FieldTitle>联网策略</FieldTitle>
      <FieldDescription>仅本地与缓存不会发起新的歌词网络请求</FieldDescription>
    </FieldContent>
    <Tabs :model-value="settings.networkPolicy" @update:model-value="selectNetworkPolicy">
      <TabsList aria-label="歌词联网策略">
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
      <FieldTitle>在线解析策略</FieldTitle>
      <FieldDescription>
        <div>并行查询：等待更短，但会同时请求多个来源</div>
        <div>当前平台优先：命中可靠逐字后停止；未命中时兜底会更慢</div>
      </FieldDescription>
    </FieldContent>
    <ToggleGroup
      type="single"
      variant="outline"
      :model-value="settings.onlineStrategy"
      :disabled="saving || !settings.enabled || settings.networkPolicy === 'local_only'"
      aria-label="在线歌词解析策略"
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
