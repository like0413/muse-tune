<script setup lang="ts">
import { Languages } from '@lucide/vue'
import type { DeepReadonly } from 'vue'

import { Badge } from '@/components/ui/badge'
import {
  formatAgeSeconds,
  formatBytes,
  getLyricsStatusLabel,
  getPlayerLabel,
  getPrecisionLabel,
  getResolutionMethodLabel,
} from '@/features/diagnostics/labels'
import type { LyricsDiagnostics, LyricsResolutionStep } from '@/features/lyrics/types'

import DiagnosticRow from './DiagnosticRow.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'

const props = defineProps<{ diagnostics: DeepReadonly<LyricsDiagnostics> }>()
const { t } = useI18n({ useScope: 'global' })

const originalSource = computed(() => {
  const source = props.diagnostics.snapshot.source
  if (!source) return t('common.unavailable')
  return t('diagnostics.lyrics.sourceValue', {
    player: getPlayerLabel(source.player),
    kind: t(source.kind === 'local' ? 'diagnostics.lyrics.local' : 'diagnostics.lyrics.online'),
  })
})

const resolverStatus = computed(() => {
  if (props.diagnostics.pendingResolution) return t('diagnostics.lyrics.pendingUpdate')
  if (props.diagnostics.resolverRunning) return t('diagnostics.values.resolving')
  return t('diagnostics.values.idle')
})

const currentCacheStatus = computed(() => {
  const cache = props.diagnostics.cache
  if (!cache.currentEntryExists) return t('diagnostics.lyrics.noCurrentCache')
  if (cache.currentEntryFresh === false) return t('diagnostics.lyrics.refreshDue')
  if (cache.currentEntryFresh === null) return t('diagnostics.lyrics.unknownState')
  const freshness =
    cache.currentRefreshRemainingSeconds === null
      ? t('diagnostics.values.valid')
      : t('diagnostics.lyrics.refreshAfter', {
          duration: formatAgeSeconds(cache.currentRefreshRemainingSeconds),
        })
  const applicationCacheMissed = props.diagnostics.resolutionSteps.some(
    (step) => step.label === 'Muse Tune 缓存' && step.outcome === 'miss',
  )

  return applicationCacheMissed ? t('diagnostics.lyrics.writtenThisTime', { freshness }) : freshness
})

const onlineStrategyLabel = computed(() =>
  props.diagnostics.onlineStrategy === 'current_player_first'
    ? t('settings.taskbar.lyrics.currentFirst')
    : t('settings.taskbar.lyrics.parallel'),
)

/** 把连续的并发步骤折叠成一个阶段，避免将同时执行的请求显示成先后顺序。 */
const resolutionStages = computed(() => {
  const stages: Array<{
    key: string
    parallelGroup: string | null
    steps: DeepReadonly<LyricsResolutionStep>[]
  }> = []
  props.diagnostics.resolutionSteps.forEach((step, index) => {
    const previous = stages.at(-1)
    if (step.parallelGroup && previous?.parallelGroup === step.parallelGroup) {
      previous.steps.push(step)
      return
    }
    stages.push({
      key: `${index}-${step.label}`,
      parallelGroup: step.parallelGroup,
      steps: [step],
    })
  })
  return stages
})
</script>

<template>
  <DiagnosticsCard
    :title="t('diagnostics.lyrics.title')"
    :description="t('diagnostics.lyrics.description')"
  >
    <template #icon><Languages class="size-4 text-amber-500" /></template>
    <template #badge>
      <Badge variant="outline">
        {{ getLyricsStatusLabel(diagnostics.snapshot.status) }}
      </Badge>
    </template>
    <DiagnosticRow
      :label="t('diagnostics.lyrics.enabled')"
      :value="
        diagnostics.enabled ? t('diagnostics.values.enabled') : t('diagnostics.values.disabled')
      "
    />
    <DiagnosticRow :label="t('diagnostics.lyrics.strategy')" :value="onlineStrategyLabel" />
    <DiagnosticRow :label="t('diagnostics.lyrics.originalSource')" :value="originalSource" />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.songId')"
      :value="diagnostics.snapshot.source?.songId ?? t('diagnostics.values.notProvided')"
    />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.method')"
      :value="getResolutionMethodLabel(diagnostics.resolutionMethod)"
    />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.precision')"
      :value="
        diagnostics.snapshot.precision
          ? getPrecisionLabel(diagnostics.snapshot.precision)
          : t('common.unavailable')
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.lineCount')"
      :value="diagnostics.snapshot.lineCount"
    />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.duration')"
      :value="
        diagnostics.resolutionDurationMs === null
          ? diagnostics.resolverRunning
            ? t('diagnostics.values.processing')
            : t('common.unavailable')
          : `${diagnostics.resolutionDurationMs} ms`
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.cacheVersion')"
      :value="diagnostics.cache.schemaVersion"
    />
    <DiagnosticRow :label="t('diagnostics.lyrics.currentCache')" :value="currentCacheStatus" />
    <DiagnosticRow
      v-if="diagnostics.cache.currentEntryExists"
      :label="t('diagnostics.lyrics.cacheAge')"
      :value="formatAgeSeconds(diagnostics.cache.currentEntryAgeSeconds)"
    />
    <DiagnosticRow
      v-if="diagnostics.cache.currentEntryExists"
      :label="t('diagnostics.lyrics.entrySize')"
      :value="formatBytes(diagnostics.cache.currentEntryBytes)"
    />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.currentPlayer')"
      :value="
        diagnostics.currentPlayer
          ? getPlayerLabel(diagnostics.currentPlayer)
          : t('common.unavailable')
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.autoDirectory')"
      :value="diagnostics.localCachePath ?? t('diagnostics.values.notFound')"
      break-all
    >
      {{ diagnostics.localCachePath ?? t('diagnostics.values.notFound') }}
      <span v-if="diagnostics.localCachePath" class="text-muted-foreground">
        {{
          t('diagnostics.parenthesized', {
            value: diagnostics.localCacheAvailable
              ? t('diagnostics.values.available')
              : t('diagnostics.values.notCreated'),
          })
        }}
      </span>
    </DiagnosticRow>
    <DiagnosticRow :label="t('diagnostics.lyrics.resolverQueue')" :value="resolverStatus" />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.fileBatches')"
      :value="
        t('diagnostics.queueSummary', {
          sent: diagnostics.watcher.enqueuedBatches,
          processed: diagnostics.watcher.processedBatches,
        })
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.pendingFileEvents')"
      :value="
        t('diagnostics.currentPeak', {
          current: diagnostics.watcher.pendingBatches,
          peak: diagnostics.watcher.pendingBatchesPeak,
        })
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.coalescedFileEvents')"
      :value="
        t('diagnostics.lyrics.coalescedValue', {
          batches: diagnostics.watcher.coalescedBatches,
          callbacks: diagnostics.watcher.callbackCount,
        })
      "
    />
    <DiagnosticRow :label="t('diagnostics.lyrics.resolutionChain')">
      <ol v-if="resolutionStages.length > 0" class="grid gap-1.5">
        <li v-for="(stage, index) in resolutionStages" :key="stage.key">
          <template v-if="stage.parallelGroup">
            <div>
              {{ index + 1 }}. {{ stage.parallelGroup }}
              <span class="text-muted-foreground">{{
                t('diagnostics.parenthesized', {
                  value: t('diagnostics.lyrics.parallelCount', { count: stage.steps.length }),
                })
              }}</span>
            </div>
            <ul class="border-border ml-3 grid gap-1 border-l pl-3">
              <li v-for="step in stage.steps" :key="step.label">
                {{ step.label }} ·
                {{
                  step.outcome === 'hit'
                    ? t('diagnostics.values.hit')
                    : step.outcome === 'miss'
                      ? t('diagnostics.values.miss')
                      : t('diagnostics.values.failed')
                }}
                <span v-if="step.detail" class="text-muted-foreground">{{
                  t('diagnostics.parenthesized', { value: step.detail })
                }}</span>
              </li>
            </ul>
          </template>
          <template v-else>
            {{ index + 1 }}. {{ stage.steps[0]?.label }} ·
            {{
              stage.steps[0]?.outcome === 'hit'
                ? t('diagnostics.values.hit')
                : stage.steps[0]?.outcome === 'miss'
                  ? t('diagnostics.values.miss')
                  : t('diagnostics.values.failed')
            }}
            <span v-if="stage.steps[0]?.detail" class="text-muted-foreground">{{
              t('diagnostics.parenthesized', { value: stage.steps[0]?.detail })
            }}</span>
          </template>
        </li>
      </ol>
      <span v-else>{{ t('common.unavailable') }}</span>
    </DiagnosticRow>
    <DiagnosticRow
      :label="t('diagnostics.lyrics.latestNote')"
      :value="diagnostics.snapshot.errorReason ?? t('common.none')"
    />
  </DiagnosticsCard>
</template>
