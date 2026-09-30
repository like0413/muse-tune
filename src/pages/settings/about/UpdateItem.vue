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
import { Progress } from '@/components/ui/progress'
import { getApplicationLocaleTag } from '@/features/i18n/locales'
import type { AvailableUpdateView, UpdateStatus } from '@/features/updater/types'

const { locale, t } = useI18n({ useScope: 'global' })

const props = defineProps<{
  status: UpdateStatus
  statusLabel: string | null
  isChecking: boolean
  isDownloading: boolean
  update: AvailableUpdateView | null
  detectedVersion: string | null
  downloadProgress: number | null
  errorMessage: string | null
  isPortable: boolean
}>()

const emit = defineEmits<{
  check: []
  install: []
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
</script>

<template>
  <Item role="region" aria-labelledby="application-update">
    <ItemHeader class="flex-wrap">
      <ItemContent>
        <div class="flex flex-wrap items-center gap-2">
          <ItemTitle id="application-update">
            <RefreshCw class="size-4 text-sky-500" />
            {{ t('settings.about.update.title') }}
          </ItemTitle>
          <Badge v-if="statusLabel" :variant="statusVariant">{{ statusLabel }}</Badge>
          <p v-if="errorMessage" class="text-destructive text-xs">{{ errorMessage }}</p>
        </div>
        <ItemDescription v-if="update || detectedVersion">
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
          </span>
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
    <ItemFooter
      v-if="isDownloading && downloadProgress !== null"
      class="flex-col items-stretch gap-3"
    >
      <Progress
        :model-value="downloadProgress"
        :aria-label="t('settings.about.update.downloadProgress')"
      />
    </ItemFooter>
  </Item>
</template>
