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

const { t } = useI18n({ useScope: 'global' })

defineProps<{
  settings: DeepReadonly<TaskbarLyricsSettings>
  saving: boolean
}>()

const emit = defineEmits<{
  updateSettings: [patch: Partial<TaskbarLyricsSettings>]
  previewTimingOffset: [values: number[] | undefined]
  commitTimingOffset: [values: number[] | undefined]
}>()

const animationOptions = computed(
  () =>
    [
      { value: 'none', label: t('common.none') },
      { value: 'up', label: t('settings.taskbar.lyrics.animationUp') },
      { value: 'fade', label: t('settings.taskbar.lyrics.animationFade') },
    ] as const,
)

/** 接收下拉框的歌词动画值。 */
function selectAnimation(value: unknown) {
  if (isTaskbarLyricsAnimation(value)) emit('updateSettings', { animation: value })
}
</script>

<template>
  <Field orientation="horizontal" :data-disabled="!settings.enabled">
    <FieldContent>
      <FieldTitle>{{ t('settings.taskbar.lyrics.offset') }}</FieldTitle>
      <FieldDescription>{{ t('settings.taskbar.lyrics.offsetDescription') }}</FieldDescription>
    </FieldContent>
    <div class="flex w-56 items-center gap-3">
      <Slider
        :model-value="[settings.timingOffsetMs]"
        :min="TASKBAR_LYRICS_TIMING_OFFSET_MIN"
        :max="TASKBAR_LYRICS_TIMING_OFFSET_MAX"
        :step="50"
        :disabled="saving || !settings.enabled"
        :aria-label="t('settings.taskbar.lyrics.offset')"
        @update:model-value="emit('previewTimingOffset', $event)"
        @value-commit="emit('commitTimingOffset', $event)"
      />
      <output class="text-muted-foreground w-16 text-right text-xs tabular-nums">
        {{ settings.timingOffsetMs > 0 ? '+' : '' }}{{ settings.timingOffsetMs }}ms
      </output>
    </div>
  </Field>

  <Field orientation="horizontal" :data-disabled="!settings.enabled">
    <FieldContent>
      <FieldTitle>{{ t('settings.taskbar.lyrics.animation') }}</FieldTitle>
    </FieldContent>
    <Select
      :model-value="settings.animation"
      :disabled="saving || !settings.enabled"
      @update:model-value="selectAnimation"
    >
      <SelectTrigger class="w-40" :aria-label="t('settings.taskbar.lyrics.animation')">
        <SelectValue :placeholder="t('settings.taskbar.lyrics.animationPlaceholder')" />
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
      <FieldLabel for="taskbar-lyrics-animation-pre-roll">{{
        t('settings.taskbar.lyrics.preRoll')
      }}</FieldLabel>
      <FieldDescription>
        <div>{{ t('settings.taskbar.lyrics.preRollOn') }}</div>
        <div>{{ t('settings.taskbar.lyrics.preRollOff') }}</div>
      </FieldDescription>
    </FieldContent>
    <Switch
      id="taskbar-lyrics-animation-pre-roll"
      :model-value="settings.animationPreRoll"
      :disabled="saving || !settings.enabled || settings.animation === 'none'"
      @update:model-value="emit('updateSettings', { animationPreRoll: $event })"
    />
  </Field>
</template>
