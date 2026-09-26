<script setup lang="ts">
import { Download, LoaderCircle, RefreshCw } from '@lucide/vue'

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
import { getApplicationLocaleTag } from '@/features/i18n/locales'
import { PROJECT_RELEASES_URL } from '@/features/project/metadata'
import { isUpdateCheckFrequency, type UpdateCheckFrequency } from '@/features/updater/settings'
import type { AvailableUpdateView, UpdateStatus } from '@/features/updater/types'

const { locale, t } = useI18n({ useScope: 'global' })

const props = defineProps<{
  status: UpdateStatus
  statusLabel: string | null
  isChecking: boolean
  isDownloading: boolean
  update: AvailableUpdateView | null
  detectedVersion: string | null
  automaticCheck: boolean
  automaticCheckSaving: boolean
  updateCheckFrequency: UpdateCheckFrequency
  updateCheckFrequencySaving: boolean
  downloadProgress: number | null
  errorMessage: string | null
  isPortable: boolean
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
  return Number.isNaN(date.getTime())
    ? props.update.date
    : date.toLocaleDateString(getApplicationLocaleTag(locale.value))
})

const statusVariant = computed<BadgeVariants['variant']>(() => {
  if (props.status === 'error') return 'destructive'
  if (props.status === 'latest') return 'success'
  if (props.status === 'available' || props.status === 'detected') return 'info'
  return 'secondary'
})

const installButtonLabel = computed(() => {
  if (props.isPortable) return t('settings.about.update.portableAction')
  if (!props.isDownloading) return t('settings.about.update.install')
  if (props.downloadProgress === null) return t('settings.about.update.downloading')
  return t('settings.about.update.downloadingProgress', {
    progress: Math.round(props.downloadProgress),
  })
})

/** 仅接受选择器声明的三个检测周期。 */
function selectUpdateCheckFrequency(value: unknown) {
  if (isUpdateCheckFrequency(value)) {
    emit('updateAutomaticCheckFrequency', value)
  }
}
</script>

<template>
  <Item>
    <ItemHeader>
      <ItemContent>
        <div class="flex flex-wrap items-center gap-2">
          <ItemTitle>
            <RefreshCw class="size-4 text-sky-500" />
            {{ t('settings.about.update.title') }}
          </ItemTitle>
          <Badge v-if="statusLabel" :variant="statusVariant">{{ statusLabel }}</Badge>
          <p v-if="errorMessage" class="text-destructive text-xs">{{ errorMessage }}</p>
        </div>
        <ItemDescription>
          <span>
            <template v-if="update">
              {{
                t('settings.about.update.versions', {
                  current: update.currentVersion,
                  latest: update.version,
                })
              }}
              <template v-if="updateDate"> · {{ updateDate }}</template>
            </template>
            <template v-else-if="detectedVersion">{{
              t('settings.about.update.detected', { version: detectedVersion })
            }}</template>
            <template v-else>{{ t('settings.about.update.source') }}</template>
          </span>
          <a :href="PROJECT_RELEASES_URL" @click.prevent="emit('openReleaseNotes')" class="ml-2">{{
            t('settings.about.update.releaseNotes')
          }}</a>
        </ItemDescription>
        <ItemDescription v-if="isPortable">
          {{ t('settings.about.update.portableDescription') }}
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
          {{ t('settings.about.update.check') }}
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
          <Label for="automatic-update-check" class="text-sm">{{
            t('settings.about.update.automatic')
          }}</Label>
          <ItemDescription>{{ t('settings.about.update.automaticDescription') }}</ItemDescription>
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
            <SelectTrigger class="w-24" :aria-label="t('settings.about.update.frequency')">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="daily">{{ t('settings.about.update.daily') }}</SelectItem>
                <SelectItem value="weekly">{{ t('settings.about.update.weekly') }}</SelectItem>
                <SelectItem value="monthly">{{ t('settings.about.update.monthly') }}</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </ItemActions>
      </Item>
      <Progress
        v-if="isDownloading && downloadProgress !== null"
        :model-value="downloadProgress"
        :aria-label="t('settings.about.update.downloadProgress')"
      />
    </ItemFooter>
  </Item>
</template>
