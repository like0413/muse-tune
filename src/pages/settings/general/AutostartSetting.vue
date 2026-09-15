<script setup lang="ts">
import { Rocket } from '@lucide/vue'
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart'
import { onActivated, shallowRef } from 'vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Switch } from '@/components/ui/switch'
import { notifySettingSaveFailed, reportBackgroundFailure } from '@/features/feedback/errors'

const { t } = useI18n({ useScope: 'global' })

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
    notifySettingSaveFailed(t('settings.general.autostart.title'), error)
  }
}

onActivated(() => {
  void loadAutostartState().catch((error) => {
    reportBackgroundFailure('读取 Windows 开机自启状态失败', error)
  })
})
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-emerald-500">
      <Rocket />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.general.autostart.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.general.autostart.description') }}</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Switch
        :aria-label="t('settings.general.autostart.title')"
        :model-value="autostartEnabled"
        @update:model-value="setAutostart"
      />
    </ItemActions>
  </Item>
</template>
