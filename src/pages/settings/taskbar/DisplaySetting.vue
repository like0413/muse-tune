<script setup lang="ts">
import { Monitor } from '@lucide/vue'
import { computed, onMounted, shallowRef } from 'vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  ALL_TASKBAR_DISPLAYS,
  applyTaskbarDisplayTarget,
  getTaskbarDisplayTarget,
  listTaskbarDisplays,
  setTaskbarDisplayTarget,
  type TaskbarDisplay,
} from '@/features/settings/display'

const taskbarDisplays = shallowRef<TaskbarDisplay[]>([])
const selectedDisplayTarget = shallowRef(ALL_TASKBAR_DISPLAYS)
const displayTargetSaving = shallowRef(false)
const selectedDisplayUnavailable = computed(
  () =>
    selectedDisplayTarget.value !== ALL_TASKBAR_DISPLAYS &&
    !taskbarDisplays.value.some((display) => display.id === selectedDisplayTarget.value),
)

/** 分别读取已保存目标与显示器列表，避免单个失败遮蔽另一个结果。 */
async function loadDisplayTarget() {
  const [target, displays] = await Promise.allSettled([
    getTaskbarDisplayTarget(),
    listTaskbarDisplays(),
  ])
  if (target.status === 'fulfilled') {
    selectedDisplayTarget.value = target.value
  } else {
    console.error('读取目标显示器失败', target.reason)
  }
  if (displays.status === 'fulfilled') {
    taskbarDisplays.value = displays.value
  } else {
    console.error('读取可用显示器失败', displays.reason)
  }
}

/** 每次展开选择器时刷新列表，兼容显示器热插拔。 */
async function refreshTaskbarDisplays(open: boolean) {
  if (!open) return

  try {
    taskbarDisplays.value = await listTaskbarDisplays()
  } catch (error) {
    console.error('刷新目标显示器失败', error)
  }
}

/** 保存目标显示器；持久化失败时同步恢复原生状态。 */
async function selectDisplayTarget(value: unknown) {
  if (
    displayTargetSaving.value ||
    typeof value !== 'string' ||
    value === selectedDisplayTarget.value
  ) {
    return
  }

  const previousTarget = selectedDisplayTarget.value
  selectedDisplayTarget.value = value
  displayTargetSaving.value = true
  try {
    await setTaskbarDisplayTarget(value)
  } catch (error) {
    selectedDisplayTarget.value = previousTarget
    console.error('切换目标显示器失败', error)
    try {
      await applyTaskbarDisplayTarget(previousTarget)
    } catch (rollbackError) {
      console.error('恢复之前的目标显示器失败', rollbackError)
    }
  } finally {
    displayTargetSaving.value = false
  }
}

onMounted(loadDisplayTarget)
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-emerald-500">
      <Monitor />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>目标显示器</ItemTitle>
      <ItemDescription>选择播放器出现在哪些显示器的任务栏</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Select
        :model-value="selectedDisplayTarget"
        :disabled="displayTargetSaving"
        @update:model-value="selectDisplayTarget"
        @update:open="refreshTaskbarDisplays"
      >
        <SelectTrigger class="w-64" aria-label="目标显示器">
          <SelectValue placeholder="选择显示器" />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            <SelectItem :value="ALL_TASKBAR_DISPLAYS">全部显示器</SelectItem>
            <SelectItem v-if="selectedDisplayUnavailable" :value="selectedDisplayTarget" disabled>
              之前选择的显示器（当前不可用）
            </SelectItem>
            <SelectItem v-for="display in taskbarDisplays" :key="display.id" :value="display.id">
              {{ display.label }}{{ display.isPrimary ? '（主显示器）' : '' }} ·
              {{ display.width }}×{{ display.height }}
            </SelectItem>
          </SelectGroup>
        </SelectContent>
      </Select>
    </ItemActions>
  </Item>
</template>
