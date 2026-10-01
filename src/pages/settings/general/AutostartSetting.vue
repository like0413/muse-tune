<script setup lang="ts">
import { Rocket } from '@lucide/vue'
import type { UnlistenFn } from '@tauri-apps/api/event'

import { Item, ItemActions, ItemContent, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Switch } from '@/components/ui/switch'
import { notifySettingSaveFailed, reportBackgroundFailure } from '@/features/feedback/errors'
import {
  getAutostartEnabled,
  listenAutostartChange,
  setAutostartEnabled,
} from '@/features/settings/autostart'

const { t } = useI18n({ useScope: 'global' })

const autostartEnabled = shallowRef(false)
const saving = shallowRef(false)
let disposed = false
let readRevision = 0
let unlisten: UnlistenFn | undefined

/** 读取 Windows 中真实的开机自启状态。 */
async function loadAutostartState() {
  const revision = ++readRevision
  const enabled = await getAutostartEnabled()
  if (!disposed && revision === readRevision) autostartEnabled.value = enabled
}

/** 更新 Windows 开机自启注册，并仅在成功后提交界面状态。 */
async function setAutostart(enabledValue: boolean) {
  if (saving.value || enabledValue === autostartEnabled.value) return
  saving.value = true
  try {
    await setAutostartEnabled(enabledValue)
    await loadAutostartState()
  } catch (error) {
    notifySettingSaveFailed(t('settings.general.autostart.title'), error)
  } finally {
    saving.value = false
  }
}

/** 读取失败统一报告，供激活设置页和托盘变化通知共用。 */
function refreshAutostartState() {
  void loadAutostartState().catch((error) => {
    reportBackgroundFailure('读取 Windows 开机自启状态失败', error)
  })
}

onMounted(async () => {
  try {
    const stopListener = await listenAutostartChange(refreshAutostartState)
    if (disposed) stopListener()
    else {
      unlisten = stopListener
      refreshAutostartState()
    }
  } catch (error) {
    reportBackgroundFailure('监听开机自启状态变化失败', error)
  }
})

onActivated(refreshAutostartState)
onUnmounted(() => {
  disposed = true
  readRevision += 1
  unlisten?.()
})
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-emerald-500">
      <Rocket />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.general.autostart.title') }}</ItemTitle>
    </ItemContent>
    <ItemActions>
      <Switch
        :aria-label="t('settings.general.autostart.title')"
        :model-value="autostartEnabled"
        :disabled="saving"
        @update:model-value="setAutostart"
      />
    </ItemActions>
  </Item>
</template>
