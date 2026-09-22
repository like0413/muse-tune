<script setup lang="ts">
import { uniq } from 'es-toolkit'
import type { DeepReadonly } from 'vue'

import { Field, FieldContent, FieldDescription, FieldTitle } from '@/components/ui/field'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import type { MediaPlayer } from '@/features/media/types'
import {
  isTaskbarLyricsNetworkPolicy,
  isTaskbarLyricsOnlineStrategy,
  type TaskbarLyricsSettings,
} from '@/features/settings/lyrics'

import LyricsOnlineSourceEditor from './LyricsOnlineSourceEditor.vue'

const { t } = useI18n({ useScope: 'global' })

const props = defineProps<{
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
      { value: 'current_player_only', label: t('settings.taskbar.lyrics.currentOnly') },
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

/** 接收编辑器产生的在线接口顺序并提交持久化。 */
function reorderOnlineSources(order: MediaPlayer[]) {
  emit('updateSettings', { onlineSourceOrder: order })
}

/** 勾选或取消一个在线接口；未勾选的接口不会发起在线请求（当前播放平台除外，由原生侧隐式补上）。 */
function toggleOnlineSource(player: MediaPlayer, enabled: boolean) {
  const enabledOnlineSources = enabled
    ? uniq([...props.settings.enabledOnlineSources, player])
    : props.settings.enabledOnlineSources.filter((item) => item !== player)
  emit('updateSettings', { enabledOnlineSources })
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
      <FieldDescription>{{ t('settings.taskbar.lyrics.parallelDescription') }}</FieldDescription>
    </FieldContent>
    <Tabs :model-value="settings.onlineStrategy" @update:model-value="selectOnlineStrategy">
      <TabsList>
        <TabsTrigger
          v-for="option in onlineStrategyOptions"
          :key="option.value"
          :value="option.value"
          :disabled="saving || !settings.enabled || settings.networkPolicy === 'local_only'"
        >
          {{ option.label }}
        </TabsTrigger>
      </TabsList>
    </Tabs>
  </Field>

  <Field
    v-if="settings.onlineStrategy === 'parallel'"
    orientation="horizontal"
    :data-disabled="!settings.enabled || settings.networkPolicy === 'local_only'"
  >
    <FieldContent>
      <FieldTitle>{{ t('settings.taskbar.lyrics.onlineSources') }}</FieldTitle>
      <FieldDescription>{{
        t('settings.taskbar.lyrics.onlineSourcesDescription')
      }}</FieldDescription>
    </FieldContent>
    <LyricsOnlineSourceEditor
      :order="settings.onlineSourceOrder"
      :enabled="settings.enabledOnlineSources"
      :disabled="saving || !settings.enabled || settings.networkPolicy === 'local_only'"
      @reorder="reorderOnlineSources"
      @toggle="toggleOnlineSource"
    />
  </Field>
</template>
