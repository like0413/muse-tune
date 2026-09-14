import { invoke } from '@tauri-apps/api/core'

import type { DiagnosticsSnapshot } from './types'

/** 读取诊断快照；是否刷新低频存储统计由调用方明确决定。 */
export function getDiagnostics(refreshStorage: boolean): Promise<DiagnosticsSnapshot> {
  return invoke<DiagnosticsSnapshot>('get_diagnostics', { refreshStorage })
}
