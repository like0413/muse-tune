<script setup lang="ts">
import { Palette } from '@lucide/vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldTitle,
} from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_THEME_COLOR,
  getTaskbarThemeColor,
  isTaskbarThemeColorSource,
  normalizeHexColor,
  setTaskbarThemeColor,
  type TaskbarThemeColor,
  type TaskbarThemeColorSource,
} from '@/features/settings/theme-color'

import CustomThemeColorFields from './theme-color/CustomThemeColorFields.vue'

const { t } = useI18n({ useScope: 'global' })

const themeSourceOptions = computed(
  () =>
    [
      { value: 'cover', label: t('settings.taskbar.theme.cover') },
      { value: 'system', label: t('settings.taskbar.theme.system') },
      { value: 'custom', label: t('common.custom') },
    ] as const satisfies ReadonlyArray<{ value: TaskbarThemeColorSource; label: string }>,
)

const selectedTheme = shallowRef<TaskbarThemeColor>({ ...DEFAULT_TASKBAR_THEME_COLOR })
const committedTheme = shallowRef<TaskbarThemeColor>({ ...DEFAULT_TASKBAR_THEME_COLOR })
const customColorDraft = shallowRef(DEFAULT_TASKBAR_THEME_COLOR.customColor)
const themeSaving = shallowRef(false)
const customColorError = shallowRef('')

/** 恢复已保存的任务栏主题色设置。 */
async function loadThemeColor() {
  try {
    const theme = await getTaskbarThemeColor()
    selectedTheme.value = theme
    committedTheme.value = { ...theme }
    customColorDraft.value = theme.customColor
  } catch (error) {
    console.error('读取任务栏主题色失败', error)
  }
}

/** 保存完整主题色设置，失败时恢复最近一次成功值。 */
async function updateThemeColor(patch: Partial<TaskbarThemeColor>) {
  if (themeSaving.value) return

  const nextTheme = { ...selectedTheme.value, ...patch }
  if (
    nextTheme.source === selectedTheme.value.source &&
    nextTheme.customColor === selectedTheme.value.customColor
  ) {
    return
  }
  selectedTheme.value = nextTheme
  themeSaving.value = true
  try {
    await setTaskbarThemeColor(nextTheme)
    committedTheme.value = { ...nextTheme }
  } catch (error) {
    selectedTheme.value = { ...committedTheme.value }
    customColorDraft.value = committedTheme.value.customColor
    notifySettingSaveFailed(t('settings.taskbar.theme.title'), error)
  } finally {
    themeSaving.value = false
  }
}

/** 切换主题色来源。 */
function selectThemeSource(value: unknown) {
  if (isTaskbarThemeColorSource(value)) void updateThemeColor({ source: value })
}

/** 更新输入草稿，允许用户在提交前输入不完整的颜色值。 */
function updateCustomColorDraft(value: string | number) {
  customColorDraft.value = String(value)
  customColorError.value = ''
}

/** 校验并保存自定义颜色。 */
function commitCustomColor() {
  const color = normalizeHexColor(customColorDraft.value)
  if (!color) {
    customColorError.value = t('settings.taskbar.theme.invalidColor')
    return
  }
  customColorError.value = ''
  customColorDraft.value = color
  void updateThemeColor({ customColor: color })
}

/** 从候选色中选择并立即保存。 */
function selectPresetColor(color: string) {
  customColorDraft.value = color
  customColorError.value = ''
  void updateThemeColor({ customColor: color })
}

onMounted(loadThemeColor)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-sky-500">
      <Palette />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.theme.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.theme.description') }}</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.theme.source') }}</FieldTitle>
            <FieldDescription>{{ t('settings.taskbar.theme.sourceDescription') }}</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedTheme.source" @update:model-value="selectThemeSource">
            <TabsList>
              <TabsTrigger
                v-for="option in themeSourceOptions"
                :key="option.value"
                :value="option.value"
                :disabled="themeSaving"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>

        <CustomThemeColorFields
          v-if="selectedTheme.source === 'custom'"
          :selected-color="selectedTheme.customColor"
          :draft="customColorDraft"
          :error="customColorError"
          :disabled="themeSaving"
          @select-color="selectPresetColor"
          @update-draft="updateCustomColorDraft"
          @commit="commitCustomColor"
        />
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
