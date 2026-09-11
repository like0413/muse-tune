<script setup lang="ts">
import { Activity } from '@lucide/vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import { Badge } from '@/components/ui/badge'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import type { LyricsStatus } from '@/features/lyrics/types'
import { useLyricsDiagnostics } from '@/features/lyrics/useLyricsDiagnostics'
import type { MediaPlayer } from '@/features/media/types'

const { diagnostics } = useLyricsDiagnostics()

const statusLabels: Record<LyricsStatus, string> = {
  loading: '解析中',
  ready: '已就绪',
  unavailable: '不可用',
  error: '错误',
}
const playerLabels: Record<MediaPlayer, string> = {
  qq_music: 'QQ 音乐',
  netease_cloud_music: '网易云音乐',
  soda_music: '汽水音乐',
  kugou_music: '酷狗音乐',
  other: '其他播放器',
}

const statusLabel = computed(() =>
  diagnostics.value ? statusLabels[diagnostics.value.snapshot.status] : '读取中',
)
const sourceLabel = computed(() => {
  const source = diagnostics.value?.snapshot.source
  if (!source) return '暂无'
  return `${playerLabels[source.player]} · ${source.kind === 'local' ? '本地' : '在线'}`
})
const precisionLabel = computed(() => {
  const precision = diagnostics.value?.snapshot.precision
  if (precision === 'word') return '逐字'
  if (precision === 'line') return '逐行'
  return '暂无'
})
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-amber-500">
      <Activity />
    </ItemMedia>
    <ItemContent>
      <ItemTitle class="gap-2">
        歌词诊断
        <Badge variant="outline">{{ statusLabel }}</Badge>
      </ItemTitle>
      <ItemDescription>只读展示自动发现、最终来源和最近失败原因</ItemDescription>
    </ItemContent>

    <template #content>
      <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 text-sm">
        <dt class="text-muted-foreground">当前播放器</dt>
        <dd>{{ diagnostics?.currentPlayer ? playerLabels[diagnostics.currentPlayer] : '暂无' }}</dd>
        <dt class="text-muted-foreground">最终来源</dt>
        <dd>{{ sourceLabel }}</dd>
        <dt class="text-muted-foreground">时间精度</dt>
        <dd>{{ precisionLabel }}</dd>
        <dt class="text-muted-foreground">自动目录</dt>
        <dd class="min-w-0 break-all">
          {{ diagnostics?.localCachePath ?? '未发现' }}
          <span v-if="diagnostics?.localCachePath" class="text-muted-foreground">
            （{{ diagnostics.localCacheAvailable ? '可用' : '尚未创建' }}）
          </span>
        </dd>
        <dt class="text-muted-foreground">解析队列</dt>
        <dd>
          {{
            diagnostics?.pendingResolution
              ? '已有更新等待处理'
              : diagnostics?.resolverRunning
                ? '正在解析'
                : '空闲'
          }}
        </dd>
        <dt class="text-muted-foreground">最近说明</dt>
        <dd>{{ diagnostics?.snapshot.errorReason ?? '无' }}</dd>
      </dl>
    </template>
  </CollapsibleItem>
</template>
