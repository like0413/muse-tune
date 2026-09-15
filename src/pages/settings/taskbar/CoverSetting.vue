<script setup lang="ts">
import { Image } from '@lucide/vue'
import { computed, onMounted, shallowRef } from 'vue'

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
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  DEFAULT_TASKBAR_COVER_APPEARANCE,
  getTaskbarCoverAppearance,
  isTaskbarCoverShape,
  isTaskbarCoverVisibility,
  setTaskbarCoverAppearance,
  type TaskbarCoverAppearance,
} from '@/features/settings/cover'

const { t } = useI18n({ useScope: 'global' })

const shapeOptions = computed(
  () =>
    [
      { value: 'square', label: t('settings.taskbar.cover.square') },
      { value: 'rounded', label: t('settings.taskbar.cover.rounded') },
      { value: 'circle', label: t('settings.taskbar.cover.circle') },
    ] as const,
)

const visibilityOptions = computed(
  () =>
    [
      { value: 'always', label: t('settings.taskbar.cover.always') },
      { value: 'normal', label: t('settings.taskbar.cover.normalOnly') },
      { value: 'lyrics', label: t('settings.taskbar.cover.lyricsOnly') },
      { value: 'hidden', label: t('settings.taskbar.cover.hidden') },
    ] as const,
)

const selectedAppearance = shallowRef<TaskbarCoverAppearance>({
  ...DEFAULT_TASKBAR_COVER_APPEARANCE,
})
const committedAppearance = shallowRef<TaskbarCoverAppearance>({
  ...DEFAULT_TASKBAR_COVER_APPEARANCE,
})
const appearanceSaving = shallowRef(false)
const coverAlwaysHidden = computed(() => selectedAppearance.value.visibility === 'hidden')

/** 恢复已保存的封面配置。 */
async function loadAppearance() {
  try {
    const appearance = await getTaskbarCoverAppearance()
    selectedAppearance.value = appearance
    committedAppearance.value = { ...appearance }
  } catch (error) {
    console.error('读取封面配置失败', error)
  }
}

/** 合并并持久化一次封面配置变更，失败时恢复最近成功值。 */
async function updateAppearance(patch: Partial<TaskbarCoverAppearance>) {
  if (appearanceSaving.value) return

  const nextAppearance = { ...selectedAppearance.value, ...patch }
  selectedAppearance.value = nextAppearance
  appearanceSaving.value = true
  try {
    await setTaskbarCoverAppearance(nextAppearance)
    committedAppearance.value = { ...nextAppearance }
  } catch (error) {
    selectedAppearance.value = { ...committedAppearance.value }
    notifySettingSaveFailed(t('settings.taskbar.cover.title'), error)
  } finally {
    appearanceSaving.value = false
  }
}

/** 接收 Tabs 的外部值并更新封面形状。 */
function selectShape(value: string | number) {
  if (isTaskbarCoverShape(value)) void updateAppearance({ shape: value })
}

/** 接收 Tabs 的外部值并更新封面显示范围。 */
function selectVisibility(value: string | number) {
  if (isTaskbarCoverVisibility(value)) void updateAppearance({ visibility: value })
}

onMounted(loadAppearance)
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-amber-500">
      <Image />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.cover.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.cover.description') }}</ItemDescription>
    </ItemContent>

    <template #content>
      <FieldGroup>
        <Field orientation="horizontal">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.cover.visible') }}</FieldTitle>
            <FieldDescription>{{
              t('settings.taskbar.cover.visibleDescription')
            }}</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedAppearance.visibility" @update:model-value="selectVisibility">
            <TabsList>
              <TabsTrigger
                v-for="option in visibilityOptions"
                :key="option.value"
                :value="option.value"
                :disabled="appearanceSaving"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>

        <Field orientation="horizontal" :data-disabled="coverAlwaysHidden">
          <FieldContent>
            <FieldTitle>{{ t('settings.taskbar.cover.shape') }}</FieldTitle>
            <FieldDescription>{{ t('settings.taskbar.cover.shapeDescription') }}</FieldDescription>
          </FieldContent>
          <Tabs :model-value="selectedAppearance.shape" @update:model-value="selectShape">
            <TabsList>
              <TabsTrigger
                v-for="option in shapeOptions"
                :key="option.value"
                :value="option.value"
                :disabled="appearanceSaving || coverAlwaysHidden"
              >
                {{ option.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </Field>

        <Field
          v-if="selectedAppearance.shape === 'circle'"
          orientation="horizontal"
          :data-disabled="coverAlwaysHidden"
        >
          <FieldContent>
            <FieldLabel for="taskbar-cover-rotate">{{
              t('settings.taskbar.cover.rotate')
            }}</FieldLabel>
            <FieldDescription>{{ t('settings.taskbar.cover.rotateDescription') }}</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-cover-rotate"
            :model-value="selectedAppearance.rotateWhenPlaying"
            :disabled="appearanceSaving || coverAlwaysHidden"
            @update:model-value="updateAppearance({ rotateWhenPlaying: $event })"
          />
        </Field>

        <Field orientation="horizontal" :data-disabled="coverAlwaysHidden">
          <FieldContent>
            <FieldLabel for="taskbar-cover-player-source">{{
              t('settings.taskbar.cover.source')
            }}</FieldLabel>
            <FieldDescription>{{ t('settings.taskbar.cover.sourceDescription') }}</FieldDescription>
          </FieldContent>
          <Switch
            id="taskbar-cover-player-source"
            :model-value="selectedAppearance.showPlayerSource"
            :disabled="appearanceSaving || coverAlwaysHidden"
            @update:model-value="updateAppearance({ showPlayerSource: $event })"
          />
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
