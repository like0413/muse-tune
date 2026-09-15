<script setup lang="ts">
import type { DeepReadonly } from 'vue'

import {
  Field,
  FieldContent,
  FieldDescription,
  FieldLabel,
  FieldTitle,
} from '@/components/ui/field'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import {
  isTaskbarLyricsAnimation,
  TASKBAR_LYRICS_TIMING_OFFSET_MAX,
  TASKBAR_LYRICS_TIMING_OFFSET_MIN,
  type TaskbarLyricsSettings,
} from '@/features/settings/lyrics'

defineProps<{
  settings: DeepReadonly<TaskbarLyricsSettings>
  saving: boolean
}>()

const emit = defineEmits<{
  updateSettings: [patch: Partial<TaskbarLyricsSettings>]
  previewTimingOffset: [values: number[] | undefined]
  commitTimingOffset: [values: number[] | undefined]
}>()

const animationOptions = [
  { value: 'none', label: '无' },
  { value: 'up', label: '向上渐变' },
] as const

/** 接收下拉框的歌词动画值。 */
function selectAnimation(value: unknown) {
  if (isTaskbarLyricsAnimation(value)) emit('updateSettings', { animation: value })
}
</script>

<template>
  <Field orientation="horizontal" :data-disabled="!settings.enabled">
    <FieldContent>
      <FieldTitle>动画效果</FieldTitle>
      <FieldDescription>切换到下一句时的移动和渐变方式</FieldDescription>
    </FieldContent>
    <Select
      :model-value="settings.animation"
      :disabled="saving || !settings.enabled"
      @update:model-value="selectAnimation"
    >
      <SelectTrigger class="w-40" aria-label="歌词动画效果">
        <SelectValue placeholder="选择动画" />
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          <SelectItem v-for="option in animationOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </SelectItem>
        </SelectGroup>
      </SelectContent>
    </Select>
  </Field>

  <Field
    orientation="horizontal"
    :data-disabled="!settings.enabled || settings.animation === 'none'"
  >
    <FieldContent>
      <FieldLabel for="taskbar-lyrics-animation-pre-roll">动画提前完成</FieldLabel>
      <FieldDescription>
        <div>开启：人声前完成动画，更跟拍，但连续演唱时上一句会提前离场；</div>
        <div>关闭：按时间戳切换，不提前上一句，但动画会与人声同时开始</div>
      </FieldDescription>
    </FieldContent>
    <Switch
      id="taskbar-lyrics-animation-pre-roll"
      :model-value="settings.animationPreRoll"
      :disabled="saving || !settings.enabled || settings.animation === 'none'"
      @update:model-value="emit('updateSettings', { animationPreRoll: $event })"
    />
  </Field>

  <Field orientation="horizontal" :data-disabled="!settings.enabled">
    <FieldContent>
      <FieldTitle>时间偏移</FieldTitle>
      <FieldDescription>校准歌词源时间；正值延后、负值提前，不改变动画提前完成</FieldDescription>
    </FieldContent>
    <div class="flex w-56 items-center gap-3">
      <Slider
        :model-value="[settings.timingOffsetMs]"
        :min="TASKBAR_LYRICS_TIMING_OFFSET_MIN"
        :max="TASKBAR_LYRICS_TIMING_OFFSET_MAX"
        :step="50"
        :disabled="saving || !settings.enabled"
        aria-label="歌词时间偏移"
        @update:model-value="emit('previewTimingOffset', $event)"
        @value-commit="emit('commitTimingOffset', $event)"
      />
      <output class="text-muted-foreground w-16 text-right text-xs tabular-nums">
        {{ settings.timingOffsetMs > 0 ? '+' : '' }}{{ settings.timingOffsetMs }}ms
      </output>
    </div>
  </Field>
</template>
