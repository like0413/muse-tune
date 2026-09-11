<script setup lang="ts">
import { AudioLines } from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import { formatDuration, playbackStatusLabels, playerLabels } from '@/features/diagnostics/labels'
import type { MediaDiagnostics } from '@/features/diagnostics/types'

import DiagnosticRow from './DiagnosticRow.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'

const props = defineProps<{ diagnostics: MediaDiagnostics }>()

const controlCapabilities = computed(() => {
  const controls = [
    props.diagnostics.canTogglePlayPause && '播放/暂停',
    props.diagnostics.canSkipPrevious && '上一首',
    props.diagnostics.canSkipNext && '下一首',
    props.diagnostics.canSeek && '定位进度',
  ].filter(Boolean)
  return controls.length > 0 ? controls.join('、') : '播放器未声明可用控制'
})
</script>

<template>
  <DiagnosticsCard title="媒体会话" description="Windows GSMTC 当前选中的播放器、元数据和控制能力">
    <template #icon><AudioLines class="size-4 text-violet-500" /></template>
    <template #badge>
      <Badge variant="outline">{{ diagnostics.sessionAvailable ? '已连接' : '无会话' }}</Badge>
    </template>
    <DiagnosticRow
      label="播放器"
      :value="diagnostics.player ? playerLabels[diagnostics.player] : '暂无'"
    />
    <DiagnosticRow
      label="播放状态"
      :value="
        diagnostics.playbackStatus ? playbackStatusLabels[diagnostics.playbackStatus] : '暂无'
      "
    />
    <DiagnosticRow label="歌曲" :value="diagnostics.title ?? '未提供'" />
    <DiagnosticRow label="歌手" :value="diagnostics.artist ?? '未提供'" />
    <DiagnosticRow
      label="时间线"
      :value="
        diagnostics.timelineAvailable
          ? `${formatDuration(diagnostics.durationMs)} · 起点 ${diagnostics.timelineStartMs ?? 0} ms`
          : '播放器未提供'
      "
    />
    <DiagnosticRow
      label="播放速率"
      :value="diagnostics.playbackRate === null ? '暂无' : `${diagnostics.playbackRate}×`"
    />
    <DiagnosticRow label="控制能力" :value="controlCapabilities" />
    <DiagnosticRow
      label="发现的会话"
      :value="diagnostics.discoveredSessionCount ?? '媒体线程不可用'"
    />
    <DiagnosticRow label="选择策略" :value="diagnostics.selectionStrategy ?? '不可用'" />
    <div v-if="diagnostics.sessions.length > 0" class="col-span-2 mt-3 space-y-2 border-t pt-3">
      <div class="text-muted-foreground text-xs font-medium">候选会话</div>
      <div
        v-for="(session, index) in diagnostics.sessions"
        :key="`${session.player}-${index}`"
        class="bg-muted/20 rounded-md border px-3 py-2"
      >
        <div class="flex flex-wrap items-center gap-2">
          <span class="text-sm font-medium">{{ playerLabels[session.player] }}</span>
          <Badge v-if="session.selected" variant="secondary">当前选中</Badge>
          <Badge variant="outline">{{ playbackStatusLabels[session.playbackStatus] }}</Badge>
          <Badge variant="outline">{{ session.timelineAvailable ? '有时间线' : '无时间线' }}</Badge>
        </div>
        <div class="text-muted-foreground mt-1 truncate text-xs">
          {{ session.title ?? '未提供曲名' }} · {{ session.artist ?? '未提供歌手' }}
        </div>
      </div>
    </div>
  </DiagnosticsCard>
</template>
