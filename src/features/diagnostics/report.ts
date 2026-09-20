import type { DiagnosticsSnapshot } from './types'

/** 生成诊断报告：原样输出快照，便于排查。 */
export function createDiagnosticsReport(diagnostics: DiagnosticsSnapshot): string {
  return JSON.stringify({ collectedAt: new Date().toISOString(), ...diagnostics }, null, 2)
}
