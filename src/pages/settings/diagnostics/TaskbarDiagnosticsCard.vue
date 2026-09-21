<script setup lang="ts">
import { PanelLeft } from '@lucide/vue'
import type { DeepReadonly } from 'vue'

import { Badge } from '@/components/ui/badge'
import type { TaskbarDiagnostics } from '@/features/diagnostics/types'
import type { TaskbarOverlapPriority, TaskbarPlacement } from '@/features/taskbar/contracts'

import DiagnosticRow from './DiagnosticRow.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'

defineProps<{ diagnostics: DeepReadonly<TaskbarDiagnostics> }>()
const { t } = useI18n({ useScope: 'global' })

/** 任务栏定位的本地化标签，键与原生枚举一一对应。 */
const placementLabels = computed<Record<TaskbarPlacement, string>>(() => ({
  auto: t('common.auto'),
  left: t('diagnostics.values.leftSide'),
  right: t('diagnostics.values.rightSide'),
}))
/** 遮挡优先级的本地化标签，键与原生枚举一一对应。 */
const priorityLabels = computed<Record<TaskbarOverlapPriority, string>>(() => ({
  bar: t('diagnostics.taskbar.barPriority'),
  taskbar: t('diagnostics.taskbar.systemPriority'),
}))

const placementLabel = (value: TaskbarPlacement) => placementLabels.value[value]
const priorityLabel = (value: TaskbarOverlapPriority) => priorityLabels.value[value]
</script>

<template>
  <DiagnosticsCard
    :title="t('diagnostics.taskbar.title')"
    :description="t('diagnostics.taskbar.description')"
  >
    <template #icon><PanelLeft class="size-4 text-emerald-500" /></template>
    <template #badge>
      <Badge variant="outline">
        {{
          diagnostics.visibleBarWindowCount > 0
            ? t('diagnostics.values.running')
            : t('diagnostics.values.notShown')
        }}
      </Badge>
    </template>
    <DiagnosticRow
      :label="t('diagnostics.taskbar.displays')"
      :value="diagnostics.detectedDisplayCount"
    />
    <DiagnosticRow :label="t('diagnostics.taskbar.windows')" :value="diagnostics.barWindowCount" />
    <DiagnosticRow
      :label="t('diagnostics.taskbar.visibleWindows')"
      :value="diagnostics.visibleBarWindowCount"
    />
    <DiagnosticRow
      :label="t('diagnostics.taskbar.contentVisibility')"
      :value="
        diagnostics.contentVisible
          ? t('diagnostics.values.allowed')
          : t('diagnostics.values.ruleHidden')
      "
    />
    <DiagnosticRow
      :label="t('diagnostics.taskbar.target')"
      :value="
        diagnostics.displayTarget === 'all'
          ? t('settings.taskbar.display.all')
          : diagnostics.displayTarget
      "
      break-all
    />
    <DiagnosticRow
      :label="t('diagnostics.taskbar.placement')"
      :value="placementLabel(diagnostics.placement)"
    />
    <DiagnosticRow
      :label="t('diagnostics.taskbar.overlap')"
      :value="priorityLabel(diagnostics.overlapPriority)"
    />
    <DiagnosticRow
      :label="t('diagnostics.taskbar.logicalWidth')"
      :value="`${diagnostics.contentWidthDip} DIP`"
    />
    <DiagnosticRow :label="t('diagnostics.taskbar.displayDetails')">
      <span v-if="diagnostics.displays.length === 0">{{ t('common.none') }}</span>
      <span v-else>
        {{
          diagnostics.displays
            .map(
              (display) =>
                `${display.label} ${display.width}×${display.height}${display.isPrimary ? t('diagnostics.values.primarySuffix') : ''}`,
            )
            .join('；')
        }}
      </span>
    </DiagnosticRow>
    <DiagnosticRow :label="t('diagnostics.taskbar.windowDetails')">
      <span v-if="diagnostics.windows.length === 0">{{ t('common.none') }}</span>
      <span v-else>
        {{
          diagnostics.windows
            .map((window) =>
              t('diagnostics.taskbar.windowState', {
                label: window.label,
                state: window.visible
                  ? t('diagnostics.values.visible')
                  : t('diagnostics.values.hidden'),
              }),
            )
            .join('；')
        }}
      </span>
    </DiagnosticRow>
  </DiagnosticsCard>
</template>
