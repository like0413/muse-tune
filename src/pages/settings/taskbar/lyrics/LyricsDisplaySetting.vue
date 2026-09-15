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
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_LYRICS_SETTINGS,
  getTaskbarLyricsSettings,
  isTaskbarLyricsAlignment,
  isTaskbarLyricsAnimation,
  isTaskbarLyricsLineMode,
  isTaskbarLyricsNetworkPolicy,
  isTaskbarLyricsOnlineStrategy,
  isTaskbarLyricsSecondaryLine,
  normalizeTaskbarLyricsSettings,
  setTaskbarLyricsSettings,
  TASKBAR_LYRICS_FONT_SIZE_MAX,
  TASKBAR_LYRICS_FONT_SIZE_MIN,
  TASKBAR_LYRICS_TIMING_OFFSET_MAX,
  TASKBAR_LYRICS_TIMING_OFFSET_MIN,
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
const secondaryLineOptions = [
  { value: 'translation_only', label: '仅翻译' },
  { value: 'next', label: '下一句' },
  { value: 'translation_or_next', label: '翻译优先' },
] as const
const networkPolicyOptions = [
  { value: 'auto', label: '自动' },
  { value: 'local_only', label: '仅本地与缓存' },
] as const
const onlineStrategyOptions = [
  { value: 'parallel', label: '并行查询' },
  { value: 'current_player_first', label: '当前平台优先' },
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
    notifySettingSaveFailed('歌词设置', error)
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

/** 接收双行次要内容的优先选择。 */
function selectSecondaryLine(value: unknown) {
  if (isTaskbarLyricsSecondaryLine(value)) void updateSettings({ secondaryLine: value })
}

/** 接收联网策略选项。 */
function selectNetworkPolicy(value: unknown) {
  if (isTaskbarLyricsNetworkPolicy(value)) void updateSettings({ networkPolicy: value })
}

/** 接收在线歌词调度策略。 */
function selectOnlineStrategy(value: unknown) {
  if (isTaskbarLyricsOnlineStrategy(value)) void updateSettings({ onlineStrategy: value })
}

/** 拖动歌词时间偏移时只更新草稿。 */
function previewTimingOffset(values: number[] | undefined) {
  const timingOffsetMs = values?.[0]
  if (timingOffsetMs === undefined) return
  selectedSettings.value = normalizeTaskbarLyricsSettings({
    ...selectedSettings.value,
    timingOffsetMs,
  })
}

/** 释放滑块后持久化歌词时间偏移。 */
function commitTimingOffset(values: number[] | undefined) {
  const timingOffsetMs = values?.[0]
  if (timingOffsetMs !== undefined) void updateSettings({ timingOffsetMs })
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
      <ItemTitle>歌词</ItemTitle>
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
            <FieldTitle>显示行数</FieldTitle>
            <FieldDescription>双行时可指定第二行优先显示的内容</FieldDescription>
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

        <Field
          orientation="horizontal"
          :data-disabled="!selectedSettings.enabled || selectedSettings.lineMode !== 'double'"
        >
          <FieldContent>
            <FieldTitle>第二行回退规则</FieldTitle>
            <FieldDescription>仅“翻译优先”会在没有翻译时显示下一句</FieldDescription>
          </FieldContent>
          <Tabs
            :model-value="selectedSettings.secondaryLine"
            @update:model-value="selectSecondaryLine"
          >
            <TabsList aria-label="双行歌词第二行优先内容">
              <TabsTrigger
                v-for="option in secondaryLineOptions"
                :key="option.value"
                :value="option.value"
                :disabled="
                  settingsSaving ||
                  !selectedSettings.enabled ||
                  selectedSettings.lineMode !== 'double'
                "
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

        <Field
          orientation="horizontal"
          :data-disabled="!selectedSettings.enabled || selectedSettings.animation === 'none'"
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
            :model-value="selectedSettings.animationPreRoll"
            :disabled="
              settingsSaving || !selectedSettings.enabled || selectedSettings.animation === 'none'
            "
            @update:model-value="updateSettings({ animationPreRoll: $event })"
          />
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.enabled">
          <FieldContent>
            <FieldTitle>时间偏移</FieldTitle>
            <FieldDescription
              >校准歌词源时间；正值延后、负值提前，不改变动画提前完成</FieldDescription
            >
          </FieldContent>
          <div class="flex w-56 items-center gap-3">
            <Slider
              :model-value="[selectedSettings.timingOffsetMs]"
              :min="TASKBAR_LYRICS_TIMING_OFFSET_MIN"
              :max="TASKBAR_LYRICS_TIMING_OFFSET_MAX"
              :step="50"
              :disabled="settingsSaving || !selectedSettings.enabled"
              aria-label="歌词时间偏移"
              @update:model-value="previewTimingOffset"
              @value-commit="commitTimingOffset"
            />
            <output class="text-muted-foreground w-16 text-right text-xs tabular-nums">
              {{ selectedSettings.timingOffsetMs > 0 ? '+' : ''
              }}{{ selectedSettings.timingOffsetMs }}ms
            </output>
          </div>
        </Field>

        <Field orientation="horizontal" :data-disabled="!selectedSettings.enabled">
          <FieldContent>
            <FieldTitle>联网策略</FieldTitle>
            <FieldDescription>仅本地与缓存不会发起新的歌词网络请求</FieldDescription>
          </FieldContent>
          <Tabs
            :model-value="selectedSettings.networkPolicy"
            @update:model-value="selectNetworkPolicy"
          >
            <TabsList aria-label="歌词联网策略">
              <TabsTrigger
                v-for="option in networkPolicyOptions"
                :key="option.value"
                :value="option.value"
                :disabled="settingsSaving || !selectedSettings.enabled"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>

        <Field
          orientation="horizontal"
          :data-disabled="
            !selectedSettings.enabled || selectedSettings.networkPolicy === 'local_only'
          "
        >
          <FieldContent>
            <FieldTitle>在线解析策略</FieldTitle>
            <FieldDescription>
              <div>并行查询：等待更短，但会同时请求多个来源</div>
              <div>当前平台优先：命中可靠逐字后停止；未命中时兜底会更慢</div>
            </FieldDescription>
          </FieldContent>
          <ToggleGroup
            type="single"
            variant="outline"
            :model-value="selectedSettings.onlineStrategy"
            :disabled="
              settingsSaving ||
              !selectedSettings.enabled ||
              selectedSettings.networkPolicy === 'local_only'
            "
            aria-label="在线歌词解析策略"
            @update:model-value="selectOnlineStrategy"
          >
            <ToggleGroupItem
              v-for="option in onlineStrategyOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </ToggleGroupItem>
          </ToggleGroup>
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
