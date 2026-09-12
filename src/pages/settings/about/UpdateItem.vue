<script setup lang="ts">
import { Download, LoaderCircle, RefreshCw } from '@lucide/vue'
import type { Update } from '@tauri-apps/plugin-updater'
import { computed } from 'vue'

import { Badge, type BadgeVariants } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemFooter,
  ItemHeader,
  ItemTitle,
} from '@/components/ui/item'
import { Label } from '@/components/ui/label'
import { Progress } from '@/components/ui/progress'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import type { UpdateCheckFrequency } from '@/features/updater/settings'
import { UPDATE_RELEASES_URL, type UpdateStatus } from '@/features/updater/useApplicationUpdater'

const props = defineProps<{
  status: UpdateStatus
  statusLabel: string | null
  isChecking: boolean
  isDownloading: boolean
  update: Update | null
  detectedVersion: string | null
  automaticCheck: boolean
  automaticCheckSaving: boolean
  updateCheckFrequency: UpdateCheckFrequency
  updateCheckFrequencySaving: boolean
  downloadProgress: number | null
  errorMessage: string | null
}>()

const emit = defineEmits<{
  check: []
  openReleaseNotes: []
  install: []
  updateAutomaticCheck: [enabled: boolean]
  updateAutomaticCheckFrequency: [frequency: UpdateCheckFrequency]
}>()

const updateDate = computed(() => {
  if (!props.update?.date) return null
  const date = new Date(props.update.date)
  return Number.isNaN(date.getTime()) ? props.update.date : date.toLocaleDateString('zh-CN')
})

const statusVariant = computed<BadgeVariants['variant']>(() => {
  if (props.status === 'error') return 'destructive'
  if (props.status === 'latest') return 'success'
  if (props.status === 'available' || props.status === 'detected') return 'info'
  return 'secondary'
})

const installButtonLabel = computed(() => {
  if (!props.isDownloading) return '下载并安装'
  if (props.downloadProgress === null) return '正在下载'
  return `正在下载 ${Math.round(props.downloadProgress)}%`
})

/** 仅接受选择器声明的三个检测周期。 */
function selectUpdateCheckFrequency(value: unknown) {
  if (value === 'daily' || value === 'weekly' || value === 'monthly') {
    emit('updateAutomaticCheckFrequency', value)
  }
}
</script>

<template>
  <Item>
    <ItemHeader>
      <ItemContent>
        <div class="flex flex-wrap items-center gap-2">
          <ItemTitle>应用更新</ItemTitle>
          <Badge v-if="statusLabel" :variant="statusVariant">{{ statusLabel }}</Badge>
          <p v-if="errorMessage" class="text-destructive text-xs">{{ errorMessage }}</p>
        </div>
        <ItemDescription>
          <span>
            <template v-if="update">
              当前 {{ update.currentVersion }} · 最新 {{ update.version }}
              <template v-if="updateDate"> · {{ updateDate }}</template>
            </template>
            <template v-else-if="detectedVersion">自动检测发现 {{ detectedVersion }}</template>
            <template v-else>通过 GitHub Releases 获取正式版本</template>
          </span>
          <a :href="UPDATE_RELEASES_URL" @click.prevent="emit('openReleaseNotes')" class="ml-2"
            >查看更新日志</a
          >
        </ItemDescription>
      </ItemContent>
      <ItemActions>
        <Button
          variant="outline"
          size="sm"
          :disabled="isChecking || isDownloading"
          @click="emit('check')"
        >
          <LoaderCircle v-if="isChecking" data-icon="inline-start" class="animate-spin" />
          <RefreshCw v-else data-icon="inline-start" />
          检查更新
        </Button>
        <Button v-if="update" size="sm" :disabled="isDownloading" @click="emit('install')">
          <LoaderCircle v-if="isDownloading" data-icon="inline-start" class="animate-spin" />
          <Download v-else data-icon="inline-start" />
          {{ installButtonLabel }}
        </Button>
      </ItemActions>
    </ItemHeader>
    <ItemFooter class="flex-col items-stretch gap-3">
      <Item variant="muted" class="w-full">
        <ItemContent class="gap-0.5">
          <Label for="automatic-update-check" class="text-sm">自动检测更新</Label>
          <ItemDescription>应用运行期间按所选周期检查</ItemDescription>
        </ItemContent>
        <ItemActions>
          <Switch
            id="automatic-update-check"
            :model-value="automaticCheck"
            :disabled="automaticCheckSaving || isDownloading"
            @update:model-value="emit('updateAutomaticCheck', $event)"
          />
          <Select
            :model-value="updateCheckFrequency"
            :disabled="!automaticCheck || updateCheckFrequencySaving || isDownloading"
            @update:model-value="selectUpdateCheckFrequency"
          >
            <SelectTrigger class="w-24" aria-label="自动检测更新频率">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="daily">每天</SelectItem>
                <SelectItem value="weekly">每周</SelectItem>
                <SelectItem value="monthly">每月</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </ItemActions>
      </Item>
      <Progress
        v-if="isDownloading && downloadProgress !== null"
        :model-value="downloadProgress"
        aria-label="更新下载进度"
      />
    </ItemFooter>
  </Item>
</template>
