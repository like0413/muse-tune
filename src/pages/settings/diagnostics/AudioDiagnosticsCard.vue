<script setup lang="ts">
import { Volume2 } from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import type { MediaDiagnostics } from '@/features/diagnostics/types'

import DiagnosticRow from './DiagnosticRow.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'

const props = defineProps<{ diagnostics: MediaDiagnostics }>()

const volumeLabel = computed(() => {
  if (props.diagnostics.volumeLevel === null) return '暂无应用音频会话'
  const volume = `${Math.round(props.diagnostics.volumeLevel * 100)}%`
  return props.diagnostics.muted ? `${volume} · 已静音` : volume
})
</script>

<template>
  <DiagnosticsCard title="音频能力" description="Windows 应用音频会话与按进程频谱采集状态">
    <template #icon><Volume2 class="size-4 text-cyan-500" /></template>
    <template #badge>
      <Badge variant="outline">{{ diagnostics.audioSessionBound ? '已绑定' : '未绑定' }}</Badge>
    </template>
    <DiagnosticRow label="应用音频" :value="volumeLabel" />
    <DiagnosticRow label="音频进程" :value="diagnostics.audioProcessId ?? '未匹配'" />
    <DiagnosticRow
      label="频谱开关"
      :value="
        diagnostics.spectrumEnabled === null
          ? '不可用'
          : diagnostics.spectrumEnabled
            ? '已开启'
            : '已关闭'
      "
    />
    <DiagnosticRow
      label="频谱采集"
      :value="
        diagnostics.spectrumActive === null
          ? '不可用'
          : diagnostics.spectrumActive
            ? '运行中'
            : '未运行'
      "
    />
    <DiagnosticRow label="媒体线程" :value="diagnostics.runtimeError ?? '正常'" />
  </DiagnosticsCard>
</template>
