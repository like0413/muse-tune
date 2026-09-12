<script setup lang="ts">
import { RefreshCw } from '@lucide/vue'
import { computed } from 'vue'

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { ItemGroup } from '@/components/ui/item'
import { Skeleton } from '@/components/ui/skeleton'
import { formatBytes } from '@/features/data-management/format'
import { useDataManagement } from '@/features/data-management/useDataManagement'

import CacheDataItem from './CacheDataItem.vue'
import ConfigDataItem from './ConfigDataItem.vue'
import LogsDataItem from './LogsDataItem.vue'

const {
  overview,
  clearing,
  resetting,
  clearingLogs,
  openingDirectory,
  cacheCleared,
  openDirectory,
  clearCache,
  resetConfig,
  clearLogFiles,
} = useDataManagement()

const configDetail = computed(() => {
  const config = overview.value?.config
  if (!config) return '暂无'
  return config.settingsFileExists ? formatBytes(config.settingsFileBytes) : '尚未创建'
})

const logsDetail = computed(() => {
  const logs = overview.value?.logs
  return logs ? `${logs.fileCount} 个文件 · ${formatBytes(logs.totalBytes)}` : '暂无'
})
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <template v-if="overview">
      <ItemGroup class="gap-3">
        <CacheDataItem
          :overview="overview.cache"
          :clearing="clearing"
          :opening="openingDirectory === 'cache'"
          :cleared="cacheCleared"
          @clear="clearCache"
          @open="openDirectory('cache')"
        />
        <ConfigDataItem
          :detail="configDetail"
          :opening="openingDirectory === 'config'"
          :resetting="resetting"
          @open="openDirectory('config')"
          @reset="resetConfig"
        />

        <LogsDataItem
          :detail="logsDetail"
          :opening="openingDirectory === 'logs'"
          :clearing="clearingLogs"
          @clear="clearLogFiles"
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
