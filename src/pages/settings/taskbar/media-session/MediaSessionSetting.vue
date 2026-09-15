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

const strategyOptions = [
  { value: 'recent_playback', label: '最近开始播放优先' },
  { value: 'sticky_current', label: '当前播放器优先' },
  { value: 'follow_windows', label: '跟随 Windows' },
  { value: 'fixed_priority', label: '固定播放器优先级' },
] as const satisfies readonly { value: MediaSessionSelectionStrategy; label: string }[]

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
    notifySettingSaveFailed('播放器抢占策略', error)
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
      <ItemTitle>播放器抢占策略</ItemTitle>
      <ItemDescription>决定多个播放器同时存在时，任务栏显示哪一个</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldTitle>抢占策略</FieldTitle>
            <FieldDescription>
              最近开始播放优先只在播放器真正播放时切换，打开客户端不会抢占
            </FieldDescription>
          </FieldContent>
          <Select
            :model-value="selectedStrategy"
            :disabled="policySaving"
            @update:model-value="selectStrategy"
          >
            <SelectTrigger class="w-56" aria-label="播放器抢占策略">
              <SelectValue placeholder="选择抢占策略" />
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
            <FieldTitle>禁止采集其他媒体</FieldTitle>
            <FieldDescription>
              开启后忽略 QQ 音乐、网易云音乐、汽水音乐和酷狗音乐以外的媒体会话
            </FieldDescription>
          </FieldContent>
          <Switch
            :model-value="onlySupportedPlayers"
            :disabled="policySaving"
            aria-label="禁止采集其他播放器"
            @update:model-value="updateOnlySupportedPlayers"
          />
        </Field>

        <Field v-if="selectedStrategy === 'fixed_priority'" orientation="horizontal">
          <FieldContent>
            <FieldTitle>播放器优先级</FieldTitle>
            <FieldDescription>多个播放器同时播放时，优先显示排序靠前的播放器</FieldDescription>
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
