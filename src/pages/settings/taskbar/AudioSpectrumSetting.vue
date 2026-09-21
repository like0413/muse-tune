<script setup lang="ts">
import { AudioLines } from '@lucide/vue'

import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldTitle,
} from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  TASKBAR_SPECTRUM_BAR_COUNT_MAX,
  TASKBAR_SPECTRUM_BAR_COUNT_MIN,
  TASKBAR_SPECTRUM_FRAME_RATES,
  TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MAX,
  TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MIN,
  TASKBAR_SPECTRUM_SENSITIVITY_MAX,
  TASKBAR_SPECTRUM_SENSITIVITY_MIN,
  TASKBAR_SPECTRUM_SMOOTHING_MAX,
  TASKBAR_SPECTRUM_SMOOTHING_MIN,
  TASKBAR_SPECTRUM_WIDTH_PERCENTAGE_MAX,
  TASKBAR_SPECTRUM_WIDTH_PERCENTAGE_MIN,
} from '@/features/settings/audio-spectrum'
import { useAudioSpectrumSetting } from '@/features/settings/controllers/useAudioSpectrumSetting'

const { t } = useI18n({ useScope: 'global' })

const alignmentOptions = computed(
  () =>
    [
      { value: 'center', label: t('common.center') },
      { value: 'bottom', label: t('common.bottomAligned') },
    ] as const,
)

const {
  selectedSettings,
  settingsSaving,
  updateSettings,
  selectAlignment,
  selectFrameRate,
  updateBarCount,
  updateWidthPercentage,
  updateHorizontalPosition,
  updateSensitivity,
  updateSmoothing,
  commitSlider,
} = useAudioSpectrumSetting()
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-fuchsia-500">
      <AudioLines />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.spectrum.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.spectrum.description') }}</ItemDescription>
    </ItemContent>
    <template #actions>
      <Switch
        :model-value="selectedSettings.visible"
        :disabled="settingsSaving"
        :aria-label="t('settings.taskbar.spectrum.visible')"
        @update:model-value="updateSettings({ visible: $event })"
      />
    </template>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal" :data-disabled="!selectedSettings.visible">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.spectrum.barCount') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.spectrum.barCountDescription')
            }}</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.barCount]"
              :min="TASKBAR_SPECTRUM_BAR_COUNT_MIN"
              :max="TASKBAR_SPECTRUM_BAR_COUNT_MAX"
              :step="1"
              :disabled="settingsSaving || !selectedSettings.visible"
              :aria-label="t('settings.taskbar.spectrum.barCount')"
              @update:model-value="updateBarCount"
              @value-commit="commitSlider"
            />
            <output class="text-muted-foreground w-10 text-right text-xs tabular-nums">
              {{ selectedSettings.barCount }}
            </output>
          </div>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.visible">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.spectrum.width') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.spectrum.widthDescription')
            }}</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.widthPercentage]"
              :min="TASKBAR_SPECTRUM_WIDTH_PERCENTAGE_MIN"
              :max="TASKBAR_SPECTRUM_WIDTH_PERCENTAGE_MAX"
              :step="1"
              :disabled="settingsSaving || !selectedSettings.visible"
              :aria-label="t('settings.taskbar.spectrum.width')"
              @update:model-value="updateWidthPercentage"
              @value-commit="commitSlider"
            />
            <output class="text-muted-foreground w-14 text-right text-xs tabular-nums">
              {{ selectedSettings.widthPercentage }}%
            </output>
          </div>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.visible">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.spectrum.horizontal') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.spectrum.horizontalDescription')
            }}</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.horizontalPosition]"
              :min="TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MIN"
              :max="TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MAX"
              :step="1"
              :disabled="settingsSaving || !selectedSettings.visible"
              :aria-label="t('settings.taskbar.spectrum.horizontal')"
              @update:model-value="updateHorizontalPosition"
              @value-commit="commitSlider"
            />
            <output class="text-muted-foreground w-10 text-right text-xs tabular-nums">
              {{ selectedSettings.horizontalPosition }}%
            </output>
          </div>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.visible">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.spectrum.vertical') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.spectrum.verticalDescription')
            }}</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedSettings.alignment" @update:model-value="selectAlignment">
            <TabsList>
              <TabsTrigger
                v-for="option in alignmentOptions"
                :key="option.value"
                :value="option.value"
                :disabled="settingsSaving || !selectedSettings.visible"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.visible">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.spectrum.sensitivity') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.spectrum.sensitivityDescription')
            }}</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.sensitivity]"
              :min="TASKBAR_SPECTRUM_SENSITIVITY_MIN"
              :max="TASKBAR_SPECTRUM_SENSITIVITY_MAX"
              :step="5"
              :disabled="settingsSaving || !selectedSettings.visible"
              :aria-label="t('settings.taskbar.spectrum.sensitivity')"
              @update:model-value="updateSensitivity"
              @value-commit="commitSlider"
            />
            <output class="text-muted-foreground w-14 text-right text-xs tabular-nums">
              {{ selectedSettings.sensitivity }}%
            </output>
          </div>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.visible">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.spectrum.smoothing') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.spectrum.smoothingDescription')
            }}</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.smoothing]"
              :min="TASKBAR_SPECTRUM_SMOOTHING_MIN"
              :max="TASKBAR_SPECTRUM_SMOOTHING_MAX"
              :step="5"
              :disabled="settingsSaving || !selectedSettings.visible"
              :aria-label="t('settings.taskbar.spectrum.smoothing')"
              @update:model-value="updateSmoothing"
              @value-commit="commitSlider"
            />
            <output class="text-muted-foreground w-14 text-right text-xs tabular-nums">
              {{ selectedSettings.smoothing }}%
            </output>
          </div>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.visible">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.spectrum.frameRate') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.spectrum.frameRateDescription')
            }}</FieldDescription>
          </FieldContent>
          <Tabs
            :model-value="String(selectedSettings.frameRate)"
            @update:model-value="selectFrameRate"
          >
            <TabsList>
              <TabsTrigger
                v-for="frameRate in TASKBAR_SPECTRUM_FRAME_RATES"
                :key="frameRate"
                :value="String(frameRate)"
                :disabled="settingsSaving || !selectedSettings.visible"
              >
                {{ frameRate }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
