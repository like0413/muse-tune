<script setup lang="ts">
import { AudioLines } from '@lucide/vue'
import { onMounted, shallowRef } from 'vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldTitle,
} from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import type {
  MediaPlayer,
  MediaSessionSelectionPolicy,
  MediaSessionSelectionStrategy,
} from '@/features/media/types'
import {
  DEFAULT_MEDIA_SESSION_SELECTION_POLICY,
  getMediaSessionSelectionPolicy,
  isMediaSessionSelectionStrategy,
  setMediaSessionSelectionPolicy,
} from '@/features/settings/media-session'

import PlayerPriorityEditor from './PlayerPriorityEditor.vue'

const { t } = useI18n({ useScope: 'global' })

const strategyOptions = computed(
  () =>
    [
      { value: 'recent_playback', label: t('settings.taskbar.mediaSession.recent') },
      { value: 'sticky_current', label: t('settings.taskbar.mediaSession.sticky') },
      { value: 'follow_windows', label: t('settings.taskbar.mediaSession.windows') },
      { value: 'fixed_priority', label: t('settings.taskbar.mediaSession.fixed') },
    ] as const satisfies readonly { value: MediaSessionSelectionStrategy; label: string }[],
)

const selectedStrategy = shallowRef(DEFAULT_MEDIA_SESSION_SELECTION_POLICY.strategy)
const selectedPriority = shallowRef<MediaPlayer[]>([
  ...DEFAULT_MEDIA_SESSION_SELECTION_POLICY.playerPriority,
])
const onlySupportedPlayers = shallowRef(DEFAULT_MEDIA_SESSION_SELECTION_POLICY.onlySupportedPlayers)
const committedPolicy = shallowRef<MediaSessionSelectionPolicy>({
  strategy: DEFAULT_MEDIA_SESSION_SELECTION_POLICY.strategy,
  playerPriority: [...DEFAULT_MEDIA_SESSION_SELECTION_POLICY.playerPriority],
  onlySupportedPlayers: DEFAULT_MEDIA_SESSION_SELECTION_POLICY.onlySupportedPlayers,
})
const policySaving = shallowRef(false)

/** 从独立响应式字段生成一次不可变的完整策略快照。 */
function createPolicy(): MediaSessionSelectionPolicy {
  return {
    strategy: selectedStrategy.value,
    playerPriority: [...selectedPriority.value],
    onlySupportedPlayers: onlySupportedPlayers.value,
  }
}

/** 用完整策略同步表单字段，避免排序工具持有失效数组。 */
function applyPolicy(policy: MediaSessionSelectionPolicy) {
  selectedStrategy.value = policy.strategy
  selectedPriority.value = [...policy.playerPriority]
  onlySupportedPlayers.value = policy.onlySupportedPlayers
}

/** 恢复已保存的播放器抢占策略。 */
async function loadPolicy() {
  try {
    const policy = await getMediaSessionSelectionPolicy()
    applyPolicy(policy)
    committedPolicy.value = {
      strategy: policy.strategy,
      playerPriority: [...policy.playerPriority],
      onlySupportedPlayers: policy.onlySupportedPlayers,
    }
  } catch (error) {
    console.error('读取播放器抢占策略失败', error)
  }
}

/** 保存完整策略，失败时恢复最近一次成功值。 */
async function savePolicy(policy: MediaSessionSelectionPolicy) {
  if (policySaving.value) return
  policySaving.value = true
  try {
    await setMediaSessionSelectionPolicy(policy)
    applyPolicy(policy)
    committedPolicy.value = {
      strategy: policy.strategy,
      playerPriority: [...policy.playerPriority],
      onlySupportedPlayers: policy.onlySupportedPlayers,
    }
  } catch (error) {
    applyPolicy(committedPolicy.value)
    notifySettingSaveFailed(t('settings.taskbar.mediaSession.title'), error)
  } finally {
    policySaving.value = false
  }
}

/** 更新策略类型，并保留用户已经排好的播放器优先级。 */
function selectStrategy(value: unknown) {
  if (!isMediaSessionSelectionStrategy(value) || value === selectedStrategy.value) return
  selectedStrategy.value = value
  void savePolicy(createPolicy())
}

/** 更新是否只采集四个已接入播放器的媒体会话。 */
function updateOnlySupportedPlayers(value: boolean) {
  if (value === onlySupportedPlayers.value) return
  onlySupportedPlayers.value = value
  void savePolicy(createPolicy())
}

/** 接收编辑器产生的新顺序并一次性保存完整策略。 */
function updatePriority(players: MediaPlayer[]) {
  if (policySaving.value) return
  selectedPriority.value = [...players]
  void savePolicy(createPolicy())
}

onMounted(loadPolicy)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-violet-500">
      <AudioLines />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.mediaSession.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.mediaSession.description') }}</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.mediaSession.strategy') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.mediaSession.strategyDescription')
            }}</FieldDescription>
          </FieldContent>
          <Select
            :model-value="selectedStrategy"
            :disabled="policySaving"
            @update:model-value="selectStrategy"
          >
            <SelectTrigger class="w-56" :aria-label="t('settings.taskbar.mediaSession.strategy')">
              <SelectValue :placeholder="t('settings.taskbar.mediaSession.placeholder')" />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem
                  v-for="strategy in strategyOptions"
                  :key="strategy.value"
                  :value="strategy.value"
                >
                  {{ strategy.label }}
                </SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </Field>

        <Field orientation="horizontal">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.mediaSession.supportedOnly') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.mediaSession.supportedOnlyDescription')
            }}</FieldDescription>
          </FieldContent>
          <Switch
            :model-value="onlySupportedPlayers"
            :disabled="policySaving"
            :aria-label="t('settings.taskbar.mediaSession.supportedOnly')"
            @update:model-value="updateOnlySupportedPlayers"
          />
        </Field>

        <Field v-if="selectedStrategy === 'fixed_priority'" orientation="horizontal">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.mediaSession.priority') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.mediaSession.priorityDescription')
            }}</FieldDescription>
          </FieldContent>
          <PlayerPriorityEditor
            :players="selectedPriority"
            :disabled="policySaving"
            @reorder="updatePriority"
          />
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
