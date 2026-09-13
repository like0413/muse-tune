<script setup lang="ts">
import { Rocket } from '@lucide/vue'
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart'
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

const autostartEnabled = shallowRef(false)

/** 读取 Windows 中真实的开机自启状态。 */
async function loadAutostartState() {
  autostartEnabled.value = await isEnabled()
}

/** 更新 Windows 开机自启注册，并仅在成功后提交界面状态。 */
async function setAutostart(enabledValue: boolean) {
  if (enabledValue === autostartEnabled.value) return
  try {
    if (enabledValue) {
      await enable()
    } else {
      await disable()
    }
    autostartEnabled.value = enabledValue
  } catch (error) {
    notifySettingSaveFailed('开机自启', error)
  }
}

onMounted(loadAutostartState)
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-emerald-500">
      <Rocket />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>开机自启</ItemTitle>
      <ItemDescription>开机后自动启动 Muse Tune</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Switch
        aria-label="开机自启"
        :model-value="autostartEnabled"
        @update:model-value="setAutostart"
      />
    </ItemActions>
  </Item>
</template>
