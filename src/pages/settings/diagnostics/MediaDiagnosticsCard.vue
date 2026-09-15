<script setup lang="ts">
import { AudioLines } from '@lucide/vue'
import type { DeepReadonly } from 'vue'

import { Badge } from '@/components/ui/badge'
import {
  formatDuration,
  getPlaybackStatusLabel,
  getPlayerLabel,
} from '@/features/diagnostics/labels'
import type { MediaDiagnostics } from '@/features/diagnostics/types'

import DiagnosticRow from './DiagnosticRow.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'

const props = defineProps<{ diagnostics: DeepReadonly<MediaDiagnostics> }>()
const { t } = useI18n({ useScope: 'global' })

const controlCapabilities = computed(() => {
  const controls = [
    props.diagnostics.canTogglePlayPause && t('media.playPause'),
    props.diagnostics.canSkipPrevious && t('media.previous'),
    props.diagnostics.canSkipNext && t('media.next'),
    props.diagnostics.canSeek && t('diagnostics.media.seek'),
  ].filter(Boolean)
  return controls.length > 0
    ? controls.join(t('diagnostics.listSeparator'))
    : t('diagnostics.media.noControls')
})

const workerMessageSummary = computed(() => {
  const worker = props.diagnostics.worker
  if (!worker) return t('diagnostics.media.workerUnavailable')
  const sent = worker.messages.reduce((total, message) => total + message.sent, 0)
  const processed = worker.messages.reduce((total, message) => total + message.processed, 0)
  return t('diagnostics.queueSummary', { sent, processed })
})
</script>

<template>
  <DiagnosticsCard
    :title="t('diagnostics.media.title')"
    :description="t('diagnostics.media.description')"
  >
    <template #icon><AudioLines class="size-4 text-violet-500" /></template>
    <template #badge>
      <Badge variant="outline">{{
        diagnostics.sessionAvailable
          ? t('diagnostics.values.connected')
          : t('diagnostics.values.noSession')
      }}</Badge>
    </template>
    <DiagnosticRow
      :label="t('diagnostics.media.player')"
      :value="diagnostics.player ? getPlayerLabel(diagnostics.player) : t('common.unavailable')"
    />
    <DiagnosticRow
      :label="t('diagnostics.media.playbackStatus')"
      :value="
        diagnostics.playbackStatus
          ? getPlaybackStatusLabel(diagnostics.playbackStatus)
          : t('common.unavailable')
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.media.song')"
      :value="diagnostics.title ?? t('diagnostics.values.notProvided')"
    />
    <DiagnosticRow
      :label="t('diagnostics.media.artist')"
      :value="diagnostics.artist ?? t('diagnostics.values.notProvided')"
    />
    <DiagnosticRow
      :label="t('diagnostics.media.timeline')"
      :value="
        diagnostics.timelineAvailable
          ? t('diagnostics.media.timelineValue', {
              duration: formatDuration(diagnostics.durationMs),
              start: diagnostics.timelineStartMs ?? 0,
            })
          : t('diagnostics.media.timelineUnavailable')
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.media.playbackRate')"
      :value="
        diagnostics.playbackRate === null ? t('common.unavailable') : `${diagnostics.playbackRate}×`
      "
    />
    <DiagnosticRow :label="t('diagnostics.media.controls')" :value="controlCapabilities" />
    <DiagnosticRow
      :label="t('diagnostics.media.discoveredSessions')"
      :value="diagnostics.discoveredSessionCount ?? t('diagnostics.media.workerUnavailable')"
    />
    <DiagnosticRow
      :label="t('diagnostics.media.selectionStrategy')"
      :value="diagnostics.selectionStrategy ?? t('diagnostics.values.unavailable')"
    />
    <DiagnosticRow :label="t('diagnostics.media.messages')" :value="workerMessageSummary" />
    <DiagnosticRow
      :label="t('diagnostics.media.pendingQueue')"
      :value="
        diagnostics.worker
          ? t('diagnostics.currentPeak', {
              current: diagnostics.worker.pendingMessages,
              peak: diagnostics.worker.pendingMessagesPeak,
            })
          : t('diagnostics.values.unavailable')
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.media.maxQueueWait')"
      :value="
        diagnostics.worker
          ? `${diagnostics.worker.maxCommandQueueWaitMs} ms`
          : t('diagnostics.values.unavailable')
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.media.coalescedEvents')"
      :value="diagnostics.worker?.coalescedEventCount ?? t('diagnostics.values.unavailable')"
    />
    <DiagnosticRow
      :label="t('diagnostics.media.metadataSettle')"
      :value="
        diagnostics.worker
          ? t('diagnostics.currentPeak', {
              current: diagnostics.worker.metadataSettlePending,
              peak: diagnostics.worker.metadataSettlePendingPeak,
            })
          : t('diagnostics.values.unavailable')
      "
    />
    <div v-if="diagnostics.sessions.length > 0" class="col-span-2 mt-3 space-y-2 border-t pt-3">
      <div class="text-muted-foreground text-xs font-medium">
        {{ t('diagnostics.media.candidates') }}
      </div>
      <div
        v-for="(session, index) in diagnostics.sessions"
        :key="`${session.player}-${index}`"
        class="bg-muted/20 rounded-md border px-3 py-2"
      >
        <div class="flex flex-wrap items-center gap-2">
          <span class="text-sm font-medium">{{ getPlayerLabel(session.player) }}</span>
          <Badge v-if="session.selected" variant="secondary">{{
            t('diagnostics.media.selected')
          }}</Badge>
          <Badge variant="outline">{{ getPlaybackStatusLabel(session.playbackStatus) }}</Badge>
          <Badge variant="outline">{{
            session.timelineAvailable
              ? t('diagnostics.media.hasTimeline')
              : t('diagnostics.media.noTimeline')
          }}</Badge>
        </div>
        <div class="text-muted-foreground mt-1 truncate text-xs">
          {{ session.title ?? t('diagnostics.media.noTitle') }} ·
          {{ session.artist ?? t('diagnostics.media.noArtist') }}
        </div>
      </div>
    </div>
  </DiagnosticsCard>
</template>
