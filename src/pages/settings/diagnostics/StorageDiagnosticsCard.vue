<script setup lang="ts">
import { Database } from '@lucide/vue'

import { formatAgeSeconds, formatBytes } from '@/features/diagnostics/labels'
import type { StorageDiagnostics } from '@/features/diagnostics/types'
import type { LyricsCacheDiagnostics } from '@/features/lyrics/types'

import DiagnosticRow from './DiagnosticRow.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'

defineProps<{
  diagnostics: StorageDiagnostics
  lyricsCache: LyricsCacheDiagnostics
}>()
</script>

<template>
  <DiagnosticsCard title="存储与缓存" description="设置文件、日志和规范化歌词缓存的当前占用">
    <template #icon><Database class="size-4 text-indigo-500" /></template>
    <DiagnosticRow
      label="设置文件"
      :value="
        diagnostics.settingsFileExists ? formatBytes(diagnostics.settingsFileBytes) : '尚未创建'
      "
    />
    <DiagnosticRow label="设置路径" :value="diagnostics.settingsFile ?? '不可用'" break-all />
    <DiagnosticRow
      label="日志"
      :value="`${diagnostics.logFileCount} 个文件 · ${formatBytes(diagnostics.logTotalBytes)}`"
    />
    <DiagnosticRow label="歌词缓存版本" :value="lyricsCache.schemaVersion" />
    <DiagnosticRow
      label="歌词缓存占用"
      :value="`${lyricsCache.entryCount} 项 · ${formatBytes(lyricsCache.totalBytes)} / ${formatBytes(lyricsCache.limitBytes)}`"
    />
    <DiagnosticRow
      label="当前歌曲缓存"
      :value="
        lyricsCache.currentEntryExists ? formatBytes(lyricsCache.currentEntryBytes) : '未命中'
      "
    />
    <DiagnosticRow
      label="当前缓存状态"
      :value="
        lyricsCache.currentEntryFresh === null
          ? '暂无'
          : lyricsCache.currentEntryFresh
            ? '有效期内'
            : '已过期'
      "
    />
    <DiagnosticRow
      label="缓存年龄"
      :value="
        lyricsCache.currentEntryAgeSeconds === null
          ? '暂无'
          : formatAgeSeconds(lyricsCache.currentEntryAgeSeconds)
      "
    />
    <DiagnosticRow
      label="下次刷新"
      :value="
        lyricsCache.currentRefreshRemainingSeconds === null
          ? '暂无'
          : `${formatAgeSeconds(lyricsCache.currentRefreshRemainingSeconds)}后到期`
      "
    />
  </DiagnosticsCard>
</template>
