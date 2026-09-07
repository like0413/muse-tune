<script setup lang="ts">
import { AudioLines, GripVertical, Music2 } from '@lucide/vue'
import { moveArrayElement, useSortable } from '@vueuse/integrations/useSortable'
import type { SortableEvent } from 'sortablejs'
import { nextTick, onMounted, shallowRef, useTemplateRef } from 'vue'

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
import { getMediaPlayerPresentation } from '@/features/media/players'
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
const committedPolicy = shallowRef<MediaSessionSelectionPolicy>({
  strategy: DEFAULT_MEDIA_SESSION_SELECTION_POLICY.strategy,
  playerPriority: [...DEFAULT_MEDIA_SESSION_SELECTION_POLICY.playerPriority],
})
const policySaving = shallowRef(false)
const priorityContainer = useTemplateRef<HTMLElement>('priorityContainer')

const { option } = useSortable(priorityContainer, selectedPriority, {
  animation: 160,
  direction: 'vertical',
  forceFallback: true,
  fallbackTolerance: 3,
  handle: '[data-drag-handle]',
  ghostClass: 'opacity-40',
  onUpdate: handlePriorityUpdate,
  watchElement: true,
})

/** 从独立响应式字段生成一次不可变的完整策略快照。 */
function createPolicy(): MediaSessionSelectionPolicy {
  return { strategy: selectedStrategy.value, playerPriority: [...selectedPriority.value] }
}

/** 用完整策略同步表单字段，避免排序工具持有失效数组。 */
function applyPolicy(policy: MediaSessionSelectionPolicy) {
  selectedStrategy.value = policy.strategy
  selectedPriority.value = [...policy.playerPriority]
}

/** 恢复已保存的播放器抢占策略。 */
async function loadPolicy() {
  try {
    const policy = await getMediaSessionSelectionPolicy()
    applyPolicy(policy)
    committedPolicy.value = {
      strategy: policy.strategy,
      playerPriority: [...policy.playerPriority],
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
    }
  } catch (error) {
    applyPolicy(committedPolicy.value)
    console.error('保存播放器抢占策略失败', error)
  } finally {
    policySaving.value = false
    await nextTick()
    option('disabled', false)
  }
}

/** 更新策略类型，并保留用户已经排好的播放器优先级。 */
function selectStrategy(value: unknown) {
  if (!isMediaSessionSelectionStrategy(value) || value === selectedStrategy.value) return
  selectedStrategy.value = value
  void savePolicy(createPolicy())
}

/** 在拖动结束后一次性保存播放器优先级。 */
function handlePriorityUpdate(event: SortableEvent) {
  if (event.oldIndex === undefined || event.newIndex === undefined || policySaving.value) return

  policySaving.value = true
  option('disabled', true)
  moveArrayElement(selectedPriority, event.oldIndex, event.newIndex, event)
  void nextTick(() => {
    policySaving.value = false
    void savePolicy(createPolicy())
  })
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
      <ItemDescription>决定多个播放器同时存在时，任务栏显示和控制哪一个</ItemDescription>
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

        <Field v-if="selectedStrategy === 'fixed_priority'" orientation="horizontal">
          <FieldContent>
            <FieldTitle>播放器优先级</FieldTitle>
            <FieldDescription>多个播放器同时播放时，优先显示排序靠前的播放器</FieldDescription>
          </FieldContent>
          <div
            ref="priorityContainer"
            class="bg-muted/50 grid w-56 gap-2 rounded-lg p-2"
            aria-label="播放器优先级"
          >
            <div
              v-for="(player, index) in selectedPriority"
              :key="player"
              class="bg-background flex items-center gap-3 rounded-md border px-3 py-2 shadow-xs"
            >
              <span class="text-muted-foreground w-4 text-center text-xs">{{ index + 1 }}</span>
              <Music2 class="size-4" aria-hidden="true" />
              <span class="text-sm font-medium">
                {{ getMediaPlayerPresentation(player).label }}
              </span>
              <button
                class="text-muted-foreground hover:text-foreground focus-visible:ring-ring ml-auto grid cursor-grab place-items-center rounded-sm outline-none focus-visible:ring-2 active:cursor-grabbing"
                type="button"
                data-drag-handle
                :disabled="policySaving"
                :aria-label="`拖动${getMediaPlayerPresentation(player).label}`"
              >
                <GripVertical class="size-4" aria-hidden="true" />
              </button>
            </div>
          </div>
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
