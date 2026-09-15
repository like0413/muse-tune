<script setup lang="ts">
import { PanelTop } from '@lucide/vue'
import { onMounted, shallowRef } from 'vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import {
  applyTaskbarPlacement,
  getTaskbarPlacement,
  isTaskbarPlacement,
  setTaskbarPlacement,
  type TaskbarPlacement,
} from '@/features/settings/placement'

const { t } = useI18n({ useScope: 'global' })

const placementOptions = computed(
  () =>
    [
      { value: 'left', label: t('settings.taskbar.placement.left') },
      { value: 'auto', label: t('settings.taskbar.placement.auto') },
      { value: 'right', label: t('settings.taskbar.placement.right') },
    ] as const satisfies ReadonlyArray<{ value: TaskbarPlacement; label: string }>,
)

const selectedPlacement = shallowRef<TaskbarPlacement>('auto')
const placementSaving = shallowRef(false)

/** 恢复已保存的播放器位置。 */
async function loadPlacement() {
  try {
    selectedPlacement.value = await getTaskbarPlacement()
  } catch (error) {
    console.error('读取播放器位置失败', error)
  }
}

/** 保存选择并让任务栏播放器立即重新定位。 */
async function selectPlacement(value: unknown) {
  if (placementSaving.value || !isTaskbarPlacement(value) || value === selectedPlacement.value) {
    return
  }

  const previousPlacement = selectedPlacement.value
  selectedPlacement.value = value
  placementSaving.value = true

  try {
    await setTaskbarPlacement(value)
  } catch (error) {
    selectedPlacement.value = previousPlacement
    notifySettingSaveFailed(t('settings.taskbar.placement.title'), error)
    try {
      await applyTaskbarPlacement(previousPlacement)
    } catch (rollbackError) {
      console.error('恢复之前的播放器位置失败', rollbackError)
    }
  } finally {
    placementSaving.value = false
  }
}

onMounted(loadPlacement)
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-violet-500">
      <PanelTop />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.placement.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.placement.description') }}</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Tabs :model-value="selectedPlacement" @update:model-value="selectPlacement">
        <TabsList>
          <TabsTrigger
            v-for="option in placementOptions"
            :key="option.value"
            :value="option.value"
            :disabled="placementSaving"
          >
            {{ option.label }}
          </TabsTrigger>
        </TabsList>
      </Tabs>
    </ItemActions>
  </Item>
</template>
