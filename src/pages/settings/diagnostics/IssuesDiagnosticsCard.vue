<script setup lang="ts">
import { CircleCheck, TriangleAlert } from '@lucide/vue'
import type { DeepReadonly } from 'vue'

import type { DiagnosticIssue } from '@/features/diagnostics/types'

const { t } = useI18n({ useScope: 'global' })

defineProps<{ issues: DeepReadonly<DiagnosticIssue[]> }>()
</script>

<template>
  <section class="rounded-lg border px-4 py-3 text-sm" aria-labelledby="diagnostic-issues-title">
    <div class="flex items-center gap-2">
      <TriangleAlert v-if="issues.length > 0" class="size-4 shrink-0 text-orange-500" />
      <CircleCheck v-else class="size-4 shrink-0 text-emerald-500" />
      <h3 id="diagnostic-issues-title" class="font-medium">{{ t('diagnostics.issues.title') }}</h3>
      <span class="text-muted-foreground">
        {{
          issues.length > 0
            ? t('diagnostics.issues.count', { count: issues.length })
            : t('diagnostics.issues.none')
        }}
      </span>
    </div>

    <ul v-if="issues.length > 0" class="mt-2 grid gap-1.5 pl-6">
      <li
        v-for="(issue, index) in issues"
        :key="`${issue.area}-${index}`"
        class="flex items-start gap-2"
      >
        <span
          :class="issue.severity === 'error' ? 'text-destructive' : 'text-orange-600'"
          class="shrink-0"
        >
          {{ issue.area }}：
        </span>
        <span class="min-w-0">{{ issue.message }}</span>
      </li>
    </ul>
  </section>
</template>
