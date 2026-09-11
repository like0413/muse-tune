<script setup lang="ts">
import { PanelLeft } from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import type { TaskbarDiagnostics } from '@/features/diagnostics/types'

import DiagnosticRow from './DiagnosticRow.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'

defineProps<{ diagnostics: TaskbarDiagnostics }>()

const placementLabels: Record<string, string> = {
  auto: '自动',
  left: '左侧',
  right: '右侧',
}
const priorityLabels: Record<string, string> = {
  bar: 'Muse Tune 优先',
  taskbarelements: '系统任务栏元素优先',
}
</script>

<template>
  <DiagnosticsCard title="任务栏" description="Windows 任务栏检测结果和 Muse Tune Bar 窗口状态">
    <template #icon><PanelLeft class="size-4 text-emerald-500" /></template>
    <template #badge>
      <Badge variant="outline">
        {{ diagnostics.visibleBarWindowCount > 0 ? '运行中' : '未显示' }}
      </Badge>
    </template>
    <DiagnosticRow label="任务栏显示器" :value="diagnostics.detectedDisplayCount" />
    <DiagnosticRow label="Bar 窗口" :value="diagnostics.barWindowCount" />
    <DiagnosticRow label="可见 Bar" :value="diagnostics.visibleBarWindowCount" />
    <DiagnosticRow
      label="内容可见性"
      :value="diagnostics.contentVisible ? '允许显示' : '按规则隐藏'"
    />
    <DiagnosticRow
      label="显示目标"
      :value="diagnostics.displayTarget === 'all' ? '全部显示器' : diagnostics.displayTarget"
      break-all
    />
    <DiagnosticRow
      label="停靠位置"
      :value="placementLabels[diagnostics.placement] ?? diagnostics.placement"
    />
    <DiagnosticRow
      label="遮挡策略"
      :value="priorityLabels[diagnostics.overlapPriority] ?? diagnostics.overlapPriority"
    />
    <DiagnosticRow label="逻辑宽度" :value="`${diagnostics.contentWidthDip} DIP`" />
    <DiagnosticRow label="显示器详情">
      <span v-if="diagnostics.displays.length === 0">无</span>
      <span v-else>
        {{
          diagnostics.displays
            .map(
              (display) =>
                `${display.label} ${display.width}×${display.height}${display.isPrimary ? '（主）' : ''}`,
            )
            .join('；')
        }}
      </span>
    </DiagnosticRow>
    <DiagnosticRow label="窗口详情">
      <span v-if="diagnostics.windows.length === 0">无</span>
      <span v-else>
        {{
          diagnostics.windows
            .map((window) => `${window.label}（${window.visible ? '可见' : '隐藏'}）`)
            .join('；')
        }}
      </span>
    </DiagnosticRow>
  </DiagnosticsCard>
</template>
