<script setup lang="ts">
import { Languages } from '@lucide/vue'
import type { DeepReadonly } from 'vue'

import { Badge } from '@/components/ui/badge'
import {
  formatAgeSeconds,
  getLyricsStatusLabel,
  getPrecisionLabel,
  getResolutionMethodLabel,
  getResolutionSiteLabel,
} from '@/features/diagnostics/labels'
import { formatBytes } from '@/features/i18n/format'
import type {
  LyricsDiagnostics,
  LyricsResolutionRecord,
  LyricsResolutionStep,
  LyricsResolutionTrack,
} from '@/features/lyrics/types'
import { getMediaPlayerLabel } from '@/features/media/players'

import DiagnosticRow from './DiagnosticRow.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'

const props = defineProps<{ diagnostics: DeepReadonly<LyricsDiagnostics> }>()
const { t } = useI18n({ useScope: 'global' })

/** 命中/未命中的紧凑标记，用于把一轮解析的步骤压成单行。 */
const OUTCOME_MARKS: Record<LyricsResolutionStep['outcome'], string> = {
  hit: '✓',
  miss: '✗',
  error: '!',
}

/** 上一轮已完成的解析：缓存命中会短路整条链路，需要它来对照上一条链路的变化。 */
const recentResolutions = computed(() => props.diagnostics.recentResolutions)

/** 把一轮解析的步骤压成单行，保留来源与命中结果，便于对照各轮差异。 */
function describeResolutionSteps(steps: readonly LyricsResolutionStep[]): string {
  if (steps.length === 0) return t('common.none')
  return steps
    .map((step) => `${getResolutionSiteLabel(step.site)} ${OUTCOME_MARKS[step.outcome]}`)
    .join(' → ')
}

/** 用本地时间显示历史记录的完成时刻。 */
function formatResolvedAt(seconds: number | null): string {
  return seconds === null ? t('common.unavailable') : new Date(seconds * 1000).toLocaleTimeString()
}

/** 把毫秒转成 m:ss，用于显示曲目时长。 */
function formatTrackDuration(milliseconds: number): string {
  const totalSeconds = Math.round(milliseconds / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  return `${minutes}:${String(totalSeconds % 60).padStart(2, '0')}`
}

/**
 * 曲目信息：标题 · 艺术家 · 时长。
 *
 * 三项就是来源匹配的依据，切歌瞬间媒体会话可能给出混搭快照（标题已换、时长未换等），
 * 那时所有来源会同时未命中，只能靠这项定罪。
 */
function describeTrack(track: DeepReadonly<LyricsResolutionTrack> | null): string {
  if (!track) return t('common.unavailable')
  const duration =
    track.durationMs === null
      ? t('diagnostics.lyrics.noDuration')
      : formatTrackDuration(track.durationMs)
  return [track.title, track.artists.join(' / '), duration].filter(Boolean).join(' · ')
}

/** 历史轮次的摘要：先给曲目信息，再给压缩后的链路，便于对比两轮之间哪一项变了。 */
function describeResolvedRound(record: DeepReadonly<LyricsResolutionRecord>): string {
  return `${describeTrack(record.track)} — ${describeResolutionSteps(record.steps)}`
}

const originalSource = computed(() => {
  const source = props.diagnostics.snapshot.source
  if (!source) return t('common.unavailable')
  return t('diagnostics.lyrics.sourceValue', {
    player: getMediaPlayerLabel(source.player),
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
    (step) => step.site === 'application_cache' && step.outcome === 'miss',
  )

  return applicationCacheMissed ? t('diagnostics.lyrics.writtenThisTime', { freshness }) : freshness
})

const onlineStrategyLabel = computed(() =>
  props.diagnostics.onlineStrategy === 'current_player_only'
    ? t('settings.taskbar.lyrics.currentOnly')
    : t('settings.taskbar.lyrics.parallel'),
)

/** 把连续的并发步骤折叠成一个阶段，避免将同时执行的请求显示成先后顺序。 */
const resolutionStages = computed(() => {
  const stages: Array<{
    key: string
    parallel: boolean
    groupLabel: string | null
    steps: DeepReadonly<LyricsResolutionStep>[]
  }> = []
  props.diagnostics.resolutionSteps.forEach((step, index) => {
    const previous = stages.at(-1)
    if (step.parallel && previous?.parallel) {
      previous.steps.push(step)
      return
    }
    stages.push({
      key: `${index}-${step.site}`,
      parallel: step.parallel,
      groupLabel: step.parallel ? t('diagnostics.lyrics.groups.online') : null,
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
          ? getMediaPlayerLabel(diagnostics.currentPlayer)
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
    <DiagnosticRow
      :label="t('diagnostics.lyrics.resolutionTrack')"
      :value="describeTrack(diagnostics.resolutionTrack)"
    />
    <DiagnosticRow :label="t('diagnostics.lyrics.resolutionChain')">
      <ol v-if="resolutionStages.length > 0" class="grid gap-1.5">
        <li v-for="(stage, index) in resolutionStages" :key="stage.key">
          <template v-if="stage.groupLabel">
            <div>
              {{ index + 1 }}. {{ stage.groupLabel }}
              <span class="text-muted-foreground">{{
                t('diagnostics.parenthesized', {
                  value: t('diagnostics.lyrics.parallelCount', { count: stage.steps.length }),
                })
              }}</span>
            </div>
            <ul class="border-border ml-3 grid gap-1 border-l pl-3">
              <!-- 兜底来源共用同一个 site，同一并行组里会出现重复值，因此按位置做键。 -->
              <li v-for="(step, stepIndex) in stage.steps" :key="stepIndex">
                {{ getResolutionSiteLabel(step.site) }} ·
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
            {{ index + 1 }}.
            {{ stage.steps[0] ? getResolutionSiteLabel(stage.steps[0].site) : t('common.none') }} ·
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
      v-for="record in recentResolutions"
      :key="`${record.finishedAtSeconds}-${record.durationMs}`"
      :label="
        t('diagnostics.lyrics.recentResolution', {
          time: formatResolvedAt(record.finishedAtSeconds),
          status: getLyricsStatusLabel(record.status),
        })
      "
      :value="describeResolvedRound(record)"
    />
    <DiagnosticRow
      :label="t('diagnostics.lyrics.latestNote')"
      :value="diagnostics.snapshot.errorReason ?? t('common.none')"
    />
  </DiagnosticsCard>
</template>
