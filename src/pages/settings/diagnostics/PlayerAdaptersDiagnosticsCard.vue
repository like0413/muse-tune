<script setup lang="ts">
import { PlugZap } from '@lucide/vue'
import type { DeepReadonly } from 'vue'

import { Badge } from '@/components/ui/badge'
import { getPlayerLabel } from '@/features/diagnostics/labels'
import type { LyricsAdapterDiagnostics } from '@/features/lyrics/types'

import DiagnosticsCard from './DiagnosticsCard.vue'

const { t } = useI18n({ useScope: 'global' })

defineProps<{ adapters: DeepReadonly<LyricsAdapterDiagnostics[]> }>()
</script>

<template>
  <DiagnosticsCard
    :title="t('diagnostics.adapters.title')"
    :description="t('diagnostics.adapters.description')"
  >
    <template #icon><PlugZap class="size-4 text-pink-500" /></template>
    <div class="col-span-2 grid gap-2">
      <div
        v-for="adapter in adapters"
        :key="adapter.player"
        class="grid grid-cols-[7rem_minmax(0,1fr)] items-center gap-3 rounded-md border p-3"
      >
        <span class="font-medium">{{ getPlayerLabel(adapter.player) }}</span>
        <span class="text-muted-foreground min-w-0 break-all">
          {{ adapter.cachePath ?? t('diagnostics.adapters.notDiscovered') }}
        </span>
        <div class="col-start-2 flex flex-wrap gap-1">
          <Badge variant="outline">{{
            adapter.cachePathAvailable
              ? t('diagnostics.adapters.available')
              : t('diagnostics.adapters.unavailable')
          }}</Badge>
          <Badge variant="outline">{{
            adapter.watcherActive
              ? t('diagnostics.adapters.watching')
              : t('diagnostics.adapters.notWatching')
          }}</Badge>
        </div>
      </div>
    </div>
  </DiagnosticsCard>
</template>
