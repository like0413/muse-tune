<script setup lang="ts">
import { AppWindow } from '@lucide/vue'

import type { ApplicationDiagnostics } from '@/features/diagnostics/types'

import DiagnosticRow from './DiagnosticRow.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'

defineProps<{ diagnostics: ApplicationDiagnostics }>()
</script>

<template>
  <DiagnosticsCard title="应用" description="当前程序版本与目标运行环境">
    <template #icon><AppWindow class="size-4 text-blue-500" /></template>
    <DiagnosticRow label="应用" :value="`${diagnostics.name} ${diagnostics.version}`" />
    <DiagnosticRow
      label="运行环境"
      :value="`${diagnostics.targetOs} · ${diagnostics.targetArch}`"
    />
    <DiagnosticRow label="构建模式" :value="diagnostics.buildProfile" />
  </DiagnosticsCard>
</template>
