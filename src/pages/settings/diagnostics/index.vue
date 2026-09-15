<script setup lang="ts">
import { Check, Copy, RefreshCw } from '@lucide/vue'

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useDiagnostics } from '@/features/diagnostics/useDiagnostics'
import { getApplicationLocaleTag } from '@/features/i18n/locales'

import AudioDiagnosticsCard from './AudioDiagnosticsCard.vue'
import IssuesDiagnosticsCard from './IssuesDiagnosticsCard.vue'
import LyricsDiagnosticsCard from './LyricsDiagnosticsCard.vue'
import MediaDiagnosticsCard from './MediaDiagnosticsCard.vue'
import PlayerAdaptersDiagnosticsCard from './PlayerAdaptersDiagnosticsCard.vue'
import TaskbarDiagnosticsCard from './TaskbarDiagnosticsCard.vue'

const { diagnostics, refreshing, errorMessage, updatedAt, reportCopied, refresh, copyReport } =
  useDiagnostics()
const { locale, t } = useI18n({ useScope: 'global' })

const updatedAtLabel = computed(
  () =>
    updatedAt.value?.toLocaleTimeString(getApplicationLocaleTag(locale.value)) ??
    t('diagnostics.notCollected'),
)
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <div class="flex items-center justify-between gap-4 px-1">
      <p class="text-muted-foreground text-sm">
        {{ t('diagnostics.autoRefresh', { time: updatedAtLabel }) }}
      </p>
      <div class="flex gap-2">
        <Button variant="outline" size="sm" :disabled="!diagnostics" @click="copyReport">
          <Check v-if="reportCopied" class="size-4" />
          <Copy v-else class="size-4" />
          {{ reportCopied ? t('common.copied') : t('diagnostics.copyReport') }}
        </Button>
        <Button variant="outline" size="sm" :disabled="refreshing" @click="refresh">
          <RefreshCw :class="['size-4', { 'animate-spin': refreshing }]" />
          {{ t('common.refresh') }}
        </Button>
      </div>
    </div>

    <Alert v-if="errorMessage" variant="destructive">
      <AlertTitle>{{ t('diagnostics.readFailed') }}</AlertTitle>
      <AlertDescription>{{ errorMessage }}</AlertDescription>
    </Alert>

    <template v-if="diagnostics">
      <IssuesDiagnosticsCard :issues="diagnostics.issues" />
      <TaskbarDiagnosticsCard :diagnostics="diagnostics.taskbar" />
      <MediaDiagnosticsCard :diagnostics="diagnostics.media" />
      <AudioDiagnosticsCard :diagnostics="diagnostics.media" />
      <PlayerAdaptersDiagnosticsCard :adapters="diagnostics.lyrics.adapters" />
      <LyricsDiagnosticsCard :diagnostics="diagnostics.lyrics" />
    </template>
    <template v-else>
      <Skeleton v-for="index in 6" :key="index" class="h-44 w-full rounded-xl" />
    </template>
  </div>
</template>
