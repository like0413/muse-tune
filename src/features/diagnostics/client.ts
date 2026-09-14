import { invoke } from '@tauri-apps/api/core'

import type { DiagnosticsSnapshot } from './types'

/** 采集诊断快照；是否刷新低频存储统计由调用方明确决定。 */
export function collectDiagnostics(refreshStorage: boolean): Promise<DiagnosticsSnapshot> {
  return invoke<DiagnosticsSnapshot>('collect_diagnostics', { refreshStorage })
}
