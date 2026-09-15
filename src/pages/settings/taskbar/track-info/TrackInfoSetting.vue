<script setup lang="ts">
import { ListMusic } from '@lucide/vue'

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
import { useTrackInfoSetting } from '@/features/settings/controllers/useTrackInfoSetting'
import {
  TASKBAR_TRACK_INFO_SCROLL_SPEED_MAX,
  TASKBAR_TRACK_INFO_SCROLL_SPEED_MIN,
} from '@/features/settings/track-info'

const alignmentOptions = [
  { value: 'left', label: '左对齐' },
  { value: 'right', label: '右对齐' },
] as const

const scrollModeOptions = [
  { value: 'loop', label: '循环' },
  { value: 'restart', label: '每次从头' },
  { value: 'alternate', label: '来回滚动' },
] as const

const {
  selectedAlignment,
  alignmentSaving,
  selectedScrolling,
  scrollingSaving,
  selectedVisible,
  visibilitySaving,
  updateVisible,
  selectAlignment,
  updateScrolling,
  selectScrollMode,
  updateScrollSpeed,
  commitScrollSpeed,
} = useTrackInfoSetting()
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
