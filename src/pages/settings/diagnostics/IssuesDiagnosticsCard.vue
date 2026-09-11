<script setup lang="ts">
import { CircleCheck, TriangleAlert } from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import type { DiagnosticIssue } from '@/features/diagnostics/types'

import DiagnosticsCard from './DiagnosticsCard.vue'

defineProps<{ issues: DiagnosticIssue[] }>()
</script>

<template>
  <DiagnosticsCard title="当前问题" description="只汇总当前仍然成立、能够直接指导排查的异常状态">
    <template #icon>
      <TriangleAlert v-if="issues.length > 0" class="size-4 text-orange-500" />
      <CircleCheck v-else class="size-4 text-emerald-500" />
    </template>
    <template #badge>
      <Badge variant="outline">{{
        issues.length > 0 ? `${issues.length} 项` : '未发现异常'
      }}</Badge>
    </template>
    <div v-if="issues.length > 0" class="col-span-2 grid gap-2">
      <div
        v-for="(issue, index) in issues"
        :key="`${issue.area}-${index}`"
        class="flex items-start gap-3 rounded-md border p-3"
      >
        <Badge :variant="issue.severity === 'error' ? 'destructive' : 'outline'">
          {{ issue.area }}
        </Badge>
        <span>{{ issue.message }}</span>
      </div>
    </div>
    <p v-else class="text-muted-foreground col-span-2 text-sm">
      当前任务栏、媒体线程和歌词解析没有报告需要处理的问题。
    </p>
  </DiagnosticsCard>
</template>
