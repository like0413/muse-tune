<script setup lang="ts">
import { Captions } from '@lucide/vue'
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
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  DEFAULT_TASKBAR_LYRICS_SETTINGS,
  getTaskbarLyricsSettings,
  isTaskbarLyricsAlignment,
  isTaskbarLyricsAnimation,
  isTaskbarLyricsLineMode,
  normalizeTaskbarLyricsSettings,
  setTaskbarLyricsSettings,
  TASKBAR_LYRICS_FONT_SIZE_MAX,
  TASKBAR_LYRICS_FONT_SIZE_MIN,
  type TaskbarLyricsSettings,
} from '@/features/settings/lyrics'

import LyricsAppearanceSettings from './LyricsAppearanceSettings.vue'

const alignmentOptions = [
  { value: 'left', label: '左' },
  { value: 'center', label: '中' },
  { value: 'right', label: '右' },
] as const
const lineModeOptions = [
  { value: 'single', label: '单行' },
  { value: 'double', label: '双行' },
] as const
const animationOptions = [
  { value: 'none', label: '无' },
  { value: 'up', label: '向上渐变' },
] as const

const selectedSettings = shallowRef<TaskbarLyricsSettings>({
  ...DEFAULT_TASKBAR_LYRICS_SETTINGS,
})
const committedSettings = shallowRef<TaskbarLyricsSettings>({
  ...DEFAULT_TASKBAR_LYRICS_SETTINGS,
})
const settingsSaving = shallowRef(false)

/** 恢复已保存的歌词显示配置。 */
async function loadSettings() {
  try {
    const settings = await getTaskbarLyricsSettings()
    selectedSettings.value = settings
    committedSettings.value = { ...settings }
  } catch (error) {
    console.error('读取歌词显示配置失败', error)
  }
}

/** 合并并保存一次配置变更，失败时恢复最近成功状态。 */
async function updateSettings(patch: Partial<TaskbarLyricsSettings>) {
  if (settingsSaving.value) return
  const next = normalizeTaskbarLyricsSettings({ ...selectedSettings.value, ...patch })
  selectedSettings.value = next
  settingsSaving.value = true
  try {
    await setTaskbarLyricsSettings(next)
    committedSettings.value = { ...next }
  } catch (error) {
    selectedSettings.value = { ...committedSettings.value }
    console.error('保存歌词显示配置失败', error)
  } finally {
    settingsSaving.value = false
  }
}

/** 接收单选组件的对齐值。 */
function selectAlignment(value: unknown) {
  if (isTaskbarLyricsAlignment(value)) void updateSettings({ alignment: value })
}

/** 接收单选组件的行数模式。 */
function selectLineMode(value: unknown) {
  if (isTaskbarLyricsLineMode(value)) void updateSettings({ lineMode: value })
}

/** 接收下拉框的歌词动画值。 */
function selectAnimation(value: unknown) {
  if (isTaskbarLyricsAnimation(value)) void updateSettings({ animation: value })
}

/** 拖动字号滑块时只更新页面草稿，避免连续写入设置文件。 */
function previewFontSize(values: number[] | undefined) {
  const fontSize = values?.[0]
  if (fontSize === undefined) return
  selectedSettings.value = normalizeTaskbarLyricsSettings({
    ...selectedSettings.value,
    fontSize,
  })
}

/** 滑块释放后持久化最终字号。 */
function commitFontSize(values: number[] | undefined) {
  const fontSize = values?.[0]
  if (fontSize !== undefined) void updateSettings({ fontSize })
}

onMounted(loadSettings)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-cyan-500">
      <Captions />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>歌词设置</ItemTitle>
      <ItemDescription>设置任务栏歌词的布局和显示</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel for="taskbar-lyrics-enabled">开启歌词</FieldLabel>
            <FieldDescription>关闭后恢复显示歌曲信息和控制按钮</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-lyrics-enabled"
            :model-value="selectedSettings.enabled"
            :disabled="settingsSaving"
            @update:model-value="updateSettings({ enabled: $event })"
          />
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.enabled">
          <FieldContent>
            <FieldTitle>对齐方式</FieldTitle>
            <FieldDescription>歌词在空白区域内的水平位置</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedSettings.alignment" @update:model-value="selectAlignment">
            <TabsList aria-label="歌词对齐方式">
              <TabsTrigger
                v-for="option in alignmentOptions"
                :key="option.value"
                :value="option.value"
                :disabled="settingsSaving || !selectedSettings.enabled"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.enabled">
          <FieldContent>
            <FieldTitle>动画效果</FieldTitle>
            <FieldDescription>切换到下一句时的移动和渐变方式</FieldDescription>
          </FieldContent>
          <Select
            :model-value="selectedSettings.animation"
            :disabled="settingsSaving || !selectedSettings.enabled"
            @update:model-value="selectAnimation"
          >
            <SelectTrigger class="w-40" aria-label="歌词动画效果">
              <SelectValue placeholder="选择动画" />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem
                  v-for="option in animationOptions"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ option.label }}
                </SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.enabled">
          <FieldContent>
            <FieldTitle>歌词大小</FieldTitle>
            <FieldDescription>字号范围为 10–18px，双行时自动收紧行距</FieldDescription>
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.fontSize]"
              :min="TASKBAR_LYRICS_FONT_SIZE_MIN"
              :max="TASKBAR_LYRICS_FONT_SIZE_MAX"
              :step="1"
              :disabled="settingsSaving || !selectedSettings.enabled"
              aria-label="歌词大小"
              @update:model-value="previewFontSize"
              @value-commit="commitFontSize"
            />
            <output class="text-muted-foreground w-12 text-right text-xs tabular-nums">
              {{ selectedSettings.fontSize }}px
            </output>
          </div>
        </Field>

        <LyricsAppearanceSettings
          :settings="selectedSettings"
          :disabled="settingsSaving || !selectedSettings.enabled"
          @update-settings="updateSettings"
        />

        <Field orientation="horizontal" :data-disabled="!selectedSettings.enabled">
          <FieldContent>
            <FieldTitle>显示行数</FieldTitle>
            <FieldDescription>双行时优先在第二行显示翻译，否则显示下一句</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedSettings.lineMode" @update:model-value="selectLineMode">
            <TabsList aria-label="歌词显示行数">
              <TabsTrigger
                v-for="option in lineModeOptions"
                :key="option.value"
                :value="option.value"
                :disabled="settingsSaving || !selectedSettings.enabled"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.enabled">
          <FieldContent>
            <FieldLabel for="taskbar-lyrics-word-highlight">逐字高亮</FieldLabel>
            <FieldDescription>仅在歌词源包含逐字时间轴时生效</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-lyrics-word-highlight"
            :model-value="selectedSettings.wordHighlight"
            :disabled="settingsSaving || !selectedSettings.enabled"
            @update:model-value="updateSettings({ wordHighlight: $event })"
          />
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
