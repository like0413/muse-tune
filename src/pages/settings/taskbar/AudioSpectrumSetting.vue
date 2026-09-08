<script setup lang="ts">
import { AudioLines } from '@lucide/vue'
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
import {
  applyTaskbarAudioSpectrumSettings,
  DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS,
  getTaskbarAudioSpectrumSettings,
  isTaskbarSpectrumAlignment,
  normalizeTaskbarAudioSpectrumSettings,
  setTaskbarAudioSpectrumSettings,
  TASKBAR_SPECTRUM_BAR_COUNT_MAX,
  TASKBAR_SPECTRUM_BAR_COUNT_MIN,
  TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MAX,
  TASKBAR_SPECTRUM_HORIZONTAL_POSITION_MIN,
  TASKBAR_SPECTRUM_MAX_WIDTH_MAX,
  TASKBAR_SPECTRUM_MAX_WIDTH_MIN,
  type TaskbarAudioSpectrumSettings,
} from '@/features/settings/audio-spectrum'

const alignmentOptions = [
  { value: 'center', label: '居中' },
  { value: 'bottom', label: '底部对齐' },
] as const

const selectedSettings = shallowRef<TaskbarAudioSpectrumSettings>({
  ...DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS,
})
const committedSettings = shallowRef<TaskbarAudioSpectrumSettings>({
  ...DEFAULT_TASKBAR_AUDIO_SPECTRUM_SETTINGS,
})
const settingsSaving = shallowRef(false)

/** 恢复已保存的频谱配置。 */
async function loadSettings() {
  try {
    const settings = await getTaskbarAudioSpectrumSettings()
    selectedSettings.value = settings
    committedSettings.value = { ...settings }
  } catch (error) {
    console.error('读取任务栏频谱配置失败', error)
  }
}

/** 限频广播滑块预览，避免高频跨窗口事件。 */
const previewSettings = useThrottleFn(
  (settings: TaskbarAudioSpectrumSettings) => {
    applyTaskbarAudioSpectrumSettings(settings).catch((error) => {
      console.error('预览任务栏频谱配置失败', error)
    })
  },
  50,
  true,
  false,
)

/** 合并并持久化一次离散配置变更，失败时恢复最近成功值。 */
async function updateSettings(patch: Partial<TaskbarAudioSpectrumSettings>) {
  if (settingsSaving.value) return
  const next = normalizeTaskbarAudioSpectrumSettings({ ...selectedSettings.value, ...patch })
  selectedSettings.value = next
  settingsSaving.value = true
  try {
    await setTaskbarAudioSpectrumSettings(next)
    committedSettings.value = { ...next }
  } catch (error) {
    selectedSettings.value = { ...committedSettings.value }
    console.error('保存任务栏频谱配置失败', error)
  } finally {
    settingsSaving.value = false
  }
}

/** 接收 Tabs 外部值并更新频谱垂直位置。 */
function selectAlignment(value: string | number) {
  if (isTaskbarSpectrumAlignment(value)) void updateSettings({ alignment: value })
}

/** 更新频谱条数草稿并实时预览。 */
function updateBarCount(values: number[] | undefined) {
  updateSliderPreview('barCount', values?.[0])
}

/** 更新频谱最大宽度草稿并实时预览。 */
function updateMaxWidth(values: number[] | undefined) {
  updateSliderPreview('maxWidth', values?.[0])
}

/** 更新频谱水平位置草稿并实时预览。 */
function updateHorizontalPosition(values: number[] | undefined) {
  updateSliderPreview('horizontalPosition', values?.[0])
}

/** 规范单个滑块值并广播完整配置。 */
function updateSliderPreview(
  key: 'barCount' | 'maxWidth' | 'horizontalPosition',
  value: number | undefined,
) {
  if (settingsSaving.value || value === undefined) return
  const next = normalizeTaskbarAudioSpectrumSettings({
    ...selectedSettings.value,
    [key]: value,
  })
  selectedSettings.value = next
  previewSettings(next)
}

/** 在滑块交互结束后持久化当前完整配置。 */
function commitSlider() {
  void updateSettings(selectedSettings.value)
}

onMounted(loadSettings)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-fuchsia-500">
      <AudioLines />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>音频频谱</ItemTitle>
      <ItemDescription>在内容后方显示当前播放器的实时频谱</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-spectrum-visible">显示频谱</FieldLabel>
            <FieldDescription>关闭后会停止播放器音频采集与频谱计算</FieldDescription>
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
            <FieldTitle>最大宽度</FieldTitle>
            <FieldDescription>频谱在 bar 内不超过此宽度</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.maxWidth]"
              :min="TASKBAR_SPECTRUM_MAX_WIDTH_MIN"
              :max="TASKBAR_SPECTRUM_MAX_WIDTH_MAX"
              :step="1"
              :disabled="settingsSaving || !selectedSettings.visible"
              aria-label="频谱最大宽度"
              @update:model-value="updateMaxWidth"
              @value-commit="commitSlider"
            />
            <output class="text-muted-foreground w-14 text-right text-xs tabular-nums">
              {{ selectedSettings.maxWidth }}px
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
            <FieldTitle>水平位置</FieldTitle>
            <FieldDescription>从 bar 左侧到右侧连续调整频谱位置</FieldDescription>
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
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
