<script setup lang="ts">
import { AudioLines } from '@lucide/vue'

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
import {
  TASKBAR_SPECTRUM_BAR_COUNT_MAX,
  TASKBAR_SPECTRUM_BAR_COUNT_MIN,
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

const alignmentOptions = [
  { value: 'center', label: '居中' },
  { value: 'bottom', label: '底部对齐' },
] as const

const {
  selectedSettings,
  settingsSaving,
  updateSettings,
  selectAlignment,
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
      <ItemTitle>频谱</ItemTitle>
      <ItemDescription>显示当前播放器的实时频谱</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-spectrum-visible">显示频谱</FieldLabel>
            <FieldDescription>关闭后会停止音频采集与频谱计算</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-spectrum-visible"
            :model-value="selectedSettings.visible"
            :disabled="settingsSaving"
            @update:model-value="updateSettings({ visible: $event })"
          />
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.visible">
          <FieldContent>
            <FieldTitle>频谱条数量</FieldTitle>
            <FieldDescription>较少更简洁，较多能呈现更细的频率变化</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.barCount]"
              :min="TASKBAR_SPECTRUM_BAR_COUNT_MIN"
              :max="TASKBAR_SPECTRUM_BAR_COUNT_MAX"
              :step="1"
              :disabled="settingsSaving || !selectedSettings.visible"
              aria-label="频谱条数量"
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
            <FieldTitle>频谱宽度</FieldTitle>
            <FieldDescription>频谱占组件总宽度的比例</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.widthPercentage]"
              :min="TASKBAR_SPECTRUM_WIDTH_PERCENTAGE_MIN"
              :max="TASKBAR_SPECTRUM_WIDTH_PERCENTAGE_MAX"
              :step="1"
              :disabled="settingsSaving || !selectedSettings.visible"
              aria-label="频谱宽度百分比"
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
            <FieldTitle>水平位置</FieldTitle>
            <FieldDescription>从组件左侧到右侧连续调整频谱位置</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.horizontalPosition]"
              :min="TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MIN"
              :max="TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MAX"
              :step="1"
              :disabled="settingsSaving || !selectedSettings.visible"
              aria-label="频谱水平位置"
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
            <FieldTitle>垂直位置</FieldTitle>
            <FieldDescription>居中时向上下扩张，底部对齐时向上生长</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedSettings.alignment" @update:model-value="selectAlignment">
            <TabsList aria-label="频谱垂直位置">
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
            <FieldTitle>灵敏度</FieldTitle>
            <FieldDescription>放大或压低频谱对输入音量的响应</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.sensitivity]"
              :min="TASKBAR_SPECTRUM_SENSITIVITY_MIN"
              :max="TASKBAR_SPECTRUM_SENSITIVITY_MAX"
              :step="5"
              :disabled="settingsSaving || !selectedSettings.visible"
              aria-label="频谱灵敏度"
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
            <FieldTitle>动态平滑</FieldTitle>
            <FieldDescription>数值越高越稳定，但快速变化的响应会更慢</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.smoothing]"
              :min="TASKBAR_SPECTRUM_SMOOTHING_MIN"
              :max="TASKBAR_SPECTRUM_SMOOTHING_MAX"
              :step="5"
              :disabled="settingsSaving || !selectedSettings.visible"
              aria-label="频谱动态平滑"
              @update:model-value="updateSmoothing"
              @value-commit="commitSlider"
            />
            <output class="text-muted-foreground w-14 text-right text-xs tabular-nums">
              {{ selectedSettings.smoothing }}%
            </output>
          </div>
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
