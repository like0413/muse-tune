<script setup lang="ts">
import { Volume2 } from '@lucide/vue'
import type { DeepReadonly } from 'vue'

import { Badge } from '@/components/ui/badge'
import type { MediaDiagnostics } from '@/features/diagnostics/types'

import DiagnosticRow from './DiagnosticRow.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'

const props = defineProps<{ diagnostics: DeepReadonly<MediaDiagnostics> }>()
const { t } = useI18n({ useScope: 'global' })

const volumeLabel = computed(() => {
  if (props.diagnostics.volumeLevel === null) return t('diagnostics.audio.noSession')
  const volume = `${Math.round(props.diagnostics.volumeLevel * 100)}%`
  return props.diagnostics.muted ? t('diagnostics.audio.muted', { volume }) : volume
})
</script>

<template>
  <DiagnosticsCard
    :title="t('diagnostics.audio.title')"
    :description="t('diagnostics.audio.description')"
  >
    <template #icon><Volume2 class="size-4 text-cyan-500" /></template>
    <template #badge>
      <Badge variant="outline">{{
        diagnostics.audioSessionBound
          ? t('diagnostics.values.bound')
          : t('diagnostics.values.unbound')
      }}</Badge>
    </template>
    <DiagnosticRow :label="t('diagnostics.audio.applicationAudio')" :value="volumeLabel" />
    <DiagnosticRow
      :label="t('diagnostics.audio.process')"
      :value="diagnostics.audioProcessId ?? t('diagnostics.values.unmatched')"
    />
    <DiagnosticRow
      :label="t('diagnostics.audio.spectrumSwitch')"
      :value="
        diagnostics.spectrumEnabled === null
          ? t('diagnostics.values.unavailable')
          : diagnostics.spectrumEnabled
            ? t('diagnostics.values.enabled')
            : t('diagnostics.values.disabled')
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.audio.spectrumCapture')"
      :value="
        diagnostics.spectrumActive === null
          ? t('diagnostics.values.unavailable')
          : diagnostics.spectrumActive
            ? t('diagnostics.values.running')
            : t('diagnostics.values.notRunning')
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.audio.mediaThread')"
      :value="diagnostics.runtimeError ?? t('diagnostics.values.normal')"
    />
  </DiagnosticsCard>
</template>
