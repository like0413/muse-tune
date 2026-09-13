<script setup lang="ts">
import { ListMusic } from '@lucide/vue'
import { useThrottleFn } from '@vueuse/core'
import { onMounted, shallowRef } from 'vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldTitle,
} from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifySettingSaveFailed } from '@/features/settings/feedback'
import {
  applyTaskbarTrackInfoScrolling,
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
  DEFAULT_TASKBAR_TRACK_INFO_SCROLLING,
  DEFAULT_TASKBAR_TRACK_INFO_VISIBLE,
  getTaskbarTrackInfoAlignment,
  getTaskbarTrackInfoScrolling,
  getTaskbarTrackInfoVisible,
  isTaskbarTrackInfoAlignment,
  isTaskbarTrackInfoScrollMode,
  normalizeTaskbarTrackInfoScrollSpeed,
  setTaskbarTrackInfoAlignment,
  setTaskbarTrackInfoScrolling,
  setTaskbarTrackInfoVisible,
  TASKBAR_TRACK_INFO_SCROLL_SPEED_MAX,
  TASKBAR_TRACK_INFO_SCROLL_SPEED_MIN,
  type TaskbarTrackInfoAlignment,
  type TaskbarTrackInfoScrolling,
} from '@/features/settings/track-info'

const SCROLL_PREVIEW_INTERVAL_MS = 50

const alignmentOptions = [
  { value: 'left', label: '左对齐' },
  { value: 'right', label: '右对齐' },
] as const

const scrollModeOptions = [
  { value: 'loop', label: '循环' },
  { value: 'restart', label: '每次从头' },
  { value: 'alternate', label: '来回滚动' },
] as const

const selectedAlignment = shallowRef<TaskbarTrackInfoAlignment>(
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
)
const committedAlignment = shallowRef<TaskbarTrackInfoAlignment>(
  DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT,
)
const alignmentSaving = shallowRef(false)
const selectedScrolling = shallowRef<TaskbarTrackInfoScrolling>({
  ...DEFAULT_TASKBAR_TRACK_INFO_SCROLLING,
})
const committedScrolling = shallowRef<TaskbarTrackInfoScrolling>({
  ...DEFAULT_TASKBAR_TRACK_INFO_SCROLLING,
})
const scrollingSaving = shallowRef(false)
const selectedVisible = shallowRef(DEFAULT_TASKBAR_TRACK_INFO_VISIBLE)
const committedVisible = shallowRef(DEFAULT_TASKBAR_TRACK_INFO_VISIBLE)
const visibilitySaving = shallowRef(false)

/** 恢复歌曲信息配置。 */
async function loadTrackInfoSettings() {
  try {
    const [visible, alignment, scrolling] = await Promise.all([
      getTaskbarTrackInfoVisible(),
      getTaskbarTrackInfoAlignment(),
      getTaskbarTrackInfoScrolling(),
    ])
    selectedVisible.value = visible
    committedVisible.value = visible
    selectedAlignment.value = alignment
    committedAlignment.value = alignment
    selectedScrolling.value = scrolling
    committedScrolling.value = { ...scrolling }
  } catch (error) {
    console.error('读取歌曲信息配置失败', error)
  }
}

/** 保存歌曲信息整体显隐，失败时恢复最近成功值。 */
async function updateVisible(visible: boolean) {
  if (visibilitySaving.value || visible === selectedVisible.value) return
  selectedVisible.value = visible
  visibilitySaving.value = true
  try {
    await setTaskbarTrackInfoVisible(visible)
    committedVisible.value = visible
  } catch (error) {
    selectedVisible.value = committedVisible.value
    notifySettingSaveFailed('歌曲信息显隐', error)
  } finally {
    visibilitySaving.value = false
  }
}

/** 保存歌曲信息对齐方式，失败时恢复最近一次成功值。 */
async function selectAlignment(value: unknown) {
  if (
    alignmentSaving.value ||
    !isTaskbarTrackInfoAlignment(value) ||
    value === selectedAlignment.value
  ) {
    return
  }

  selectedAlignment.value = value
  alignmentSaving.value = true
  try {
    await setTaskbarTrackInfoAlignment(value)
    committedAlignment.value = value
  } catch (error) {
    selectedAlignment.value = committedAlignment.value
    notifySettingSaveFailed('歌曲信息对齐方式', error)
  } finally {
    alignmentSaving.value = false
  }
}

/** 合并并持久化一次滚动配置变更。 */
async function updateScrolling(patch: Partial<TaskbarTrackInfoScrolling>, restorePreview = false) {
  if (scrollingSaving.value) return

  const nextScrolling = { ...selectedScrolling.value, ...patch }
  selectedScrolling.value = nextScrolling
  scrollingSaving.value = true
  try {
    await setTaskbarTrackInfoScrolling(nextScrolling)
    committedScrolling.value = { ...nextScrolling }
  } catch (error) {
    selectedScrolling.value = { ...committedScrolling.value }
    notifySettingSaveFailed('歌名滚动配置', error)
    if (restorePreview) {
      try {
        await applyTaskbarTrackInfoScrolling(committedScrolling.value)
      } catch (rollbackError) {
        console.error('恢复之前的歌名滚动配置失败', rollbackError)
      }
    }
  } finally {
    scrollingSaving.value = false
  }
}

/** 接收选项卡的外部值并更新滚动方式。 */
function selectScrollMode(value: unknown) {
  if (isTaskbarTrackInfoScrollMode(value)) void updateScrolling({ mode: value })
}

/** 读取 Slider 的单个有效速度值。 */
function getScrollSpeed(values: number[] | undefined): number | undefined {
  return normalizeTaskbarTrackInfoScrollSpeed(values?.[0])
}

/** 限频发送速度预览，避免拖动时产生过密的跨窗口事件。 */
const previewScrollSpeed = useThrottleFn(
  (speed: number) => {
    applyTaskbarTrackInfoScrolling({ ...selectedScrolling.value, speed }).catch((error) => {
      console.error('预览歌名滚动速度失败', error)
    })
  },
  SCROLL_PREVIEW_INTERVAL_MS,
  true,
  false,
)

/** 更新速度状态并实时预览，不写入持久化存储。 */
function updateScrollSpeed(values: number[] | undefined) {
  if (scrollingSaving.value) return

  const speed = getScrollSpeed(values)
  if (speed === undefined) return

  selectedScrolling.value = { ...selectedScrolling.value, speed }
  previewScrollSpeed(speed)
}

/** 在拖动结束后持久化最终速度。 */
function commitScrollSpeed(values: number[]) {
  const speed = getScrollSpeed(values)
  if (speed !== undefined) void updateScrolling({ speed }, true)
}

onMounted(loadTrackInfoSettings)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-violet-500">
      <ListMusic />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>歌曲信息</ItemTitle>
      <ItemDescription>设置歌名对齐及溢出后的滚动表现</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-track-info-visible">显示歌曲信息</FieldLabel>
            <FieldDescription>整体隐藏歌名与歌手，并把空间留给其他组件</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-track-info-visible"
            :model-value="selectedVisible"
            :disabled="visibilitySaving"
            @update:model-value="updateVisible"
          />
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedVisible">
          <FieldContent>
            <FieldTitle>对齐方式</FieldTitle>
            <FieldDescription>调整歌名和歌手在可用区域内的对齐方向</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedAlignment" @update:model-value="selectAlignment">
            <TabsList aria-label="歌曲信息对齐方式">
              <TabsTrigger
                v-for="option in alignmentOptions"
                :key="option.value"
                :value="option.value"
                :disabled="alignmentSaving || !selectedVisible"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedVisible">
          <FieldContent>
            <FieldLabel for="taskbar-track-title-scroll">歌名超出时滚动</FieldLabel>
            <FieldDescription>关闭后超出部分显示为省略号</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-track-title-scroll"
            :model-value="selectedScrolling.enabled"
            :disabled="scrollingSaving || !selectedVisible"
            @update:model-value="updateScrolling({ enabled: $event })"
          />
        </Field>

        <Field
          orientation="horizontal"
          :data-disabled="!selectedVisible || !selectedScrolling.enabled || scrollingSaving"
        >
          <FieldContent>
            <FieldTitle>滚动速度</FieldTitle>
            <FieldDescription>按每秒移动的像素数控制滚动快慢</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedScrolling.speed]"
              :min="TASKBAR_TRACK_INFO_SCROLL_SPEED_MIN"
              :max="TASKBAR_TRACK_INFO_SCROLL_SPEED_MAX"
              :step="1"
              :disabled="!selectedVisible || !selectedScrolling.enabled || scrollingSaving"
              aria-label="歌名滚动速度"
              @update:model-value="updateScrollSpeed"
              @value-commit="commitScrollSpeed"
            />
            <output class="text-muted-foreground w-16 text-right text-xs tabular-nums">
              {{ selectedScrolling.speed }}px/s
            </output>
          </div>
        </Field>

        <Field
          orientation="horizontal"
          :data-disabled="!selectedVisible || !selectedScrolling.enabled || scrollingSaving"
        >
          <FieldContent>
            <FieldTitle>滚动方式</FieldTitle>
            <FieldDescription>设置歌名到达滚动边界后的行为</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedScrolling.mode" @update:model-value="selectScrollMode">
            <TabsList aria-label="歌名滚动方式">
              <TabsTrigger
                v-for="option in scrollModeOptions"
                :key="option.value"
                :value="option.value"
                :disabled="!selectedVisible || !selectedScrolling.enabled || scrollingSaving"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
