<script setup lang="ts">
import { Accessibility } from '@lucide/vue'
import { onMounted, shallowRef } from 'vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Switch } from '@/components/ui/switch'
import { notifySettingSaveFailed } from '@/features/settings/feedback'
import {
  getReducedMotionOverride,
  setReducedMotionOverride,
} from '@/features/settings/reduced-motion'

const enabled = shallowRef(false)
const saving = shallowRef(false)

/** 恢复应用级动态效果覆盖项。 */
onMounted(async () => {
  try {
    enabled.value = await getReducedMotionOverride()
  } catch (error) {
    console.error('读取减少动态效果设置失败', error)
  }
})

/** 保存覆盖项；关闭后仍尊重 Windows 的减少动态效果设置。 */
async function updateEnabled(next: boolean) {
  if (saving.value || next === enabled.value) return
  const previous = enabled.value
  enabled.value = next
  saving.value = true
  try {
    await setReducedMotionOverride(next)
  } catch (error) {
    enabled.value = previous
    notifySettingSaveFailed('减少动态效果', error)
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-sky-500"><Accessibility /></ItemMedia>
    <ItemContent>
      <ItemTitle>减少动态效果</ItemTitle>
      <ItemDescription>强制关闭歌词、封面与界面过渡；关闭时仍跟随 Windows 设置</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Switch
        aria-label="减少动态效果"
        :model-value="enabled"
        :disabled="saving"
        @update:model-value="updateEnabled"
      />
    </ItemActions>
  </Item>
</template>
