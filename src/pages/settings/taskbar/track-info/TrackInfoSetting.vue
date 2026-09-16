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

const { t } = useI18n({ useScope: 'global' })

const alignmentOptions = computed(
  () =>
    [
      { value: 'left', label: t('common.leftAligned') },
      { value: 'right', label: t('common.rightAligned') },
    ] as const,
)

const scrollModeOptions = computed(
  () =>
    [
      { value: 'loop', label: t('settings.taskbar.trackInfo.loop') },
      { value: 'restart', label: t('settings.taskbar.trackInfo.restart') },
      { value: 'alternate', label: t('settings.taskbar.trackInfo.alternate') },
    ] as const,
)

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
      <ItemTitle>{{ t('settings.taskbar.trackInfo.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.trackInfo.description') }}</ItemDescription>
    </ItemContent>
    <template #actions>
      <Switch
        :model-value="selectedVisible"
        :disabled="visibilitySaving"
        :aria-label="t('settings.taskbar.trackInfo.visible')"
        @update:model-value="updateVisible"
      />
    </template>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal" :data-disabled="!selectedVisible">
          <FieldContent>
            <FieldTitle>{{ t('common.alignment') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.trackInfo.alignmentDescription')
            }}</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedAlignment" @update:model-value="selectAlignment">
            <TabsList>
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
            <FieldLabel for="taskbar-track-title-scroll">{{
              t('settings.taskbar.trackInfo.scroll')
            }}</FieldLabel>
            <FieldDescription>{{
              t('settings.taskbar.trackInfo.scrollDescription')
            }}</FieldDescription>
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
            <FieldTitle>{{ t('settings.taskbar.trackInfo.speed') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.trackInfo.speedDescription')
            }}</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedScrolling.speed]"
              :min="TASKBAR_TRACK_INFO_SCROLL_SPEED_MIN"
              :max="TASKBAR_TRACK_INFO_SCROLL_SPEED_MAX"
              :step="1"
              :disabled="!selectedVisible || !selectedScrolling.enabled || scrollingSaving"
              :aria-label="t('settings.taskbar.trackInfo.speed')"
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
            <FieldTitle>{{ t('settings.taskbar.trackInfo.mode') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.trackInfo.modeDescription')
            }}</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedScrolling.mode" @update:model-value="selectScrollMode">
            <TabsList>
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
