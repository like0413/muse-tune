<script setup lang="ts">
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { ItemGroup } from '@/components/ui/item'
import { Skeleton } from '@/components/ui/skeleton'
import { useDataManagement } from '@/features/data-management/useDataManagement'

import CacheDataItem from './CacheDataItem.vue'
import ConfigDataItem from './ConfigDataItem.vue'
import LogsDataItem from './LogsDataItem.vue'

const { t } = useI18n({ useScope: 'global' })

const {
  overview,
  clearing,
  clearingCurrent,
  refreshingCurrent,
  resetting,
  clearingLogs,
  openingDirectory,
  cacheCleared,
  logsCleared,
  openDirectory,
  clearCache,
  clearCurrentCache,
  refreshCurrentLyricsData,
  resetConfig,
  clearLogHistoryFiles,
  errorMessage,
} = useDataManagement()
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <Alert v-if="errorMessage" variant="destructive">
      <AlertTitle>{{ t('feedback.operationFailed') }}</AlertTitle>
      <AlertDescription>{{ errorMessage }}</AlertDescription>
    </Alert>
    <template v-if="overview">
      <ItemGroup class="gap-3">
        <CacheDataItem
          :overview="overview.cache"
          :clearing="clearing"
          :opening="openingDirectory === 'cache'"
          :cleared="cacheCleared"
          :clearing-current="clearingCurrent"
          :refreshing-current="refreshingCurrent"
          @clear="clearCache"
          @open="openDirectory('cache')"
          @clear-current="clearCurrentCache"
          @refresh-current="refreshCurrentLyricsData"
        />
        <ConfigDataItem
          :overview="overview.config"
          :opening="openingDirectory === 'config'"
          :resetting="resetting"
          @open="openDirectory('config')"
          @reset="resetConfig"
        />

        <LogsDataItem
          :overview="overview.logs"
          :opening="openingDirectory === 'logs'"
          :clearing="clearingLogs"
          :cleared="logsCleared"
          :has-history="overview.logs.fileCount > 1"
          @clear="clearLogHistoryFiles"
          @open="openDirectory('logs')"
        />
      </ItemGroup>
    </template>
    <template v-else>
      <Skeleton class="h-28 w-full rounded-xl" />
      <Skeleton v-for="index in 2" :key="index" class="h-20 w-full rounded-xl" />
    </template>
  </div>
</template>
