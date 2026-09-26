<script setup lang="ts">
import { Cpu } from '@lucide/vue'
import { invoke } from '@tauri-apps/api/core'

import { Button } from '@/components/ui/button'
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemFooter,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Switch } from '@/components/ui/switch'
import { notifySettingSaveFailed, reportBackgroundFailure } from '@/features/feedback/errors'
import {
  getDisableGpuAcceleration,
  getEffectiveDisableGpuAcceleration,
  setDisableGpuAcceleration,
} from '@/features/settings/disable-gpu-acceleration'

const { t } = useI18n({ useScope: 'global' })

/** 持久化开关值。 */
const enabled = shallowRef(false)
/** 当前会话实际生效值（启动时捕获，重启前稳定）。 */
const effective = shallowRef(false)
const saving = shallowRef(false)
const restarting = shallowRef(false)

/** 恢复持久化值与会话生效值，用于判断是否需要提示重启。
 * 获取会话生效值失败不应拖垮开关显示——它只影响是否出现重启提示。 */
onMounted(async () => {
  try {
    enabled.value = await getDisableGpuAcceleration()
  } catch (error) {
    reportBackgroundFailure('读取禁用 GPU 加速设置失败', error)
  }
  try {
    effective.value = await getEffectiveDisableGpuAcceleration()
  } catch {
    effective.value = false
  }
})

/** 保存开关值；本会话仍使用启动时的 GPU 设置，重启后应用到新窗口。 */
async function updateEnabled(next: boolean) {
  if (saving.value || next === enabled.value) return
  const previous = enabled.value
  enabled.value = next
  saving.value = true
  try {
    await setDisableGpuAcceleration(next)
  } catch (error) {
    enabled.value = previous
    notifySettingSaveFailed(t('settings.general.disableGpuAcceleration.title'), error)
  } finally {
    saving.value = false
  }
}

/** 立即重启应用，让新的 GPU 加速设置生效。 */
async function restart() {
  if (restarting.value) return
  restarting.value = true
  try {
    await invoke('restart_application')
  } catch (error) {
    restarting.value = false
    reportBackgroundFailure('重启应用失败', error)
  }
}
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-teal-500"><Cpu /></ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.general.disableGpuAcceleration.title') }}</ItemTitle>
      <ItemDescription>{{
        t('settings.general.disableGpuAcceleration.description')
      }}</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Switch
        :aria-label="t('settings.general.disableGpuAcceleration.title')"
        :model-value="enabled"
        :disabled="saving"
        @update:model-value="updateEnabled"
      />
    </ItemActions>
    <ItemFooter v-if="enabled !== effective">
      <ItemDescription class="text-xs">
        {{ t('settings.general.disableGpuAcceleration.restartRequired') }}
      </ItemDescription>
      <Button variant="outline" size="sm" :disabled="restarting" @click="restart">
        {{ t('settings.general.disableGpuAcceleration.restartNow') }}
      </Button>
    </ItemFooter>
  </Item>
</template>
