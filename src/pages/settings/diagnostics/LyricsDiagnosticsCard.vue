<script setup lang="ts">
import { Languages } from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import {
  formatAgeSeconds,
  formatBytes,
  lyricsStatusLabels,
  playerLabels,
  precisionLabels,
  resolutionMethodLabels,
} from '@/features/diagnostics/labels'
import type { LyricsDiagnostics } from '@/features/lyrics/types'

import DiagnosticRow from './DiagnosticRow.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'

const props = defineProps<{ diagnostics: LyricsDiagnostics }>()

const originalSource = computed(() => {
  const source = props.diagnostics.snapshot.source
  if (!source) return '暂无'
  return `${playerLabels[source.player]} · ${source.kind === 'local' ? '播放器本地' : '在线接口'}`
})

const resolverStatus = computed(() => {
  if (props.diagnostics.pendingResolution) return '已有更新等待处理'
  if (props.diagnostics.resolverRunning) return '正在解析'
  return '空闲'
})

const currentCacheStatus = computed(() => {
  const cache = props.diagnostics.cache
  if (!cache.currentEntryExists) return '当前歌曲无缓存'
  if (cache.currentEntryFresh === false) return '已到刷新时间'
  if (cache.currentEntryFresh === null) return '状态未知'
  return cache.currentRefreshRemainingSeconds === null
    ? '有效'
    : `${formatAgeSeconds(cache.currentRefreshRemainingSeconds)}后刷新`
})
</script>

<template>
  <DiagnosticsCard title="歌词" description="区分歌词原始来源与本次播放的实际获取方式">
    <template #icon><Languages class="size-4 text-amber-500" /></template>
    <template #badge>
      <Badge variant="outline">
        {{ lyricsStatusLabels[diagnostics.snapshot.status] }}
      </Badge>
    </template>
    <DiagnosticRow label="歌词开关" :value="diagnostics.enabled ? '已开启' : '已关闭'" />
    <DiagnosticRow label="原始来源" :value="originalSource" />
    <DiagnosticRow label="来源歌曲 ID" :value="diagnostics.snapshot.source?.songId ?? '未提供'" />
    <DiagnosticRow label="本次获取" :value="resolutionMethodLabels[diagnostics.resolutionMethod]" />
    <DiagnosticRow
      label="时间精度"
      :value="
        diagnostics.snapshot.precision ? precisionLabels[diagnostics.snapshot.precision] : '暂无'
      "
    />
    <DiagnosticRow label="有效行数" :value="diagnostics.snapshot.lineCount" />
    <DiagnosticRow
      label="解析耗时"
      :value="
        diagnostics.resolutionDurationMs === null
          ? diagnostics.resolverRunning
            ? '处理中'
            : '暂无'
          : `${diagnostics.resolutionDurationMs} ms`
      "
    />
    <DiagnosticRow label="缓存版本" :value="diagnostics.cache.schemaVersion" />
    <DiagnosticRow label="当前缓存" :value="currentCacheStatus" />
    <DiagnosticRow
      v-if="diagnostics.cache.currentEntryExists"
      label="缓存年龄"
      :value="formatAgeSeconds(diagnostics.cache.currentEntryAgeSeconds)"
    />
    <DiagnosticRow
      v-if="diagnostics.cache.currentEntryExists"
      label="条目大小"
      :value="formatBytes(diagnostics.cache.currentEntryBytes)"
    />
    <DiagnosticRow
      label="当前播放器"
      :value="diagnostics.currentPlayer ? playerLabels[diagnostics.currentPlayer] : '暂无'"
    />
    <DiagnosticRow label="自动目录" :value="diagnostics.localCachePath ?? '未发现'" break-all>
      {{ diagnostics.localCachePath ?? '未发现' }}
      <span v-if="diagnostics.localCachePath" class="text-muted-foreground">
        （{{ diagnostics.localCacheAvailable ? '可用' : '尚未创建' }}）
      </span>
    </DiagnosticRow>
    <DiagnosticRow label="解析队列" :value="resolverStatus" />
    <DiagnosticRow label="解析链路">
      <ol v-if="diagnostics.resolutionSteps.length > 0" class="grid gap-1">
        <li v-for="(step, index) in diagnostics.resolutionSteps" :key="`${index}-${step.label}`">
          {{ index + 1 }}. {{ step.label }} ·
          {{ step.outcome === 'hit' ? '命中' : step.outcome === 'miss' ? '未命中' : '失败' }}
          <span v-if="step.detail" class="text-muted-foreground">（{{ step.detail }}）</span>
        </li>
      </ol>
      <span v-else>暂无</span>
    </DiagnosticRow>
    <DiagnosticRow label="最近说明" :value="diagnostics.snapshot.errorReason ?? '无'" />
  </DiagnosticsCard>
</template>
