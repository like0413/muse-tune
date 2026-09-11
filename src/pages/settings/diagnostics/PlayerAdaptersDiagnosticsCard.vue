<script setup lang="ts">
import { PlugZap } from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import { playerLabels } from '@/features/diagnostics/labels'
import type { LyricsAdapterDiagnostics } from '@/features/lyrics/types'

import DiagnosticsCard from './DiagnosticsCard.vue'

defineProps<{ adapters: LyricsAdapterDiagnostics[] }>()
</script>

<template>
  <DiagnosticsCard
    title="播放器适配器"
    description="四家当前版本播放器的自动目录发现与文件事件监听"
  >
    <template #icon><PlugZap class="size-4 text-pink-500" /></template>
    <div class="col-span-2 grid gap-2">
      <div
        v-for="adapter in adapters"
        :key="adapter.player"
        class="grid grid-cols-[7rem_minmax(0,1fr)] items-center gap-3 rounded-md border p-3"
      >
        <span class="font-medium">{{ playerLabels[adapter.player] }}</span>
        <span class="text-muted-foreground min-w-0 break-all">
          {{ adapter.cachePath ?? '未自动发现目录' }}
        </span>
        <div class="col-start-2 flex flex-wrap gap-1">
          <Badge variant="outline">{{
            adapter.cachePathAvailable ? '目录可用' : '目录不可用'
          }}</Badge>
          <Badge variant="outline">{{ adapter.watcherActive ? '监听中' : '未监听' }}</Badge>
        </div>
      </div>
    </div>
  </DiagnosticsCard>
</template>
