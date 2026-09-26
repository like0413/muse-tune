import { invoke } from '@tauri-apps/api/core'

export interface RuntimeEnvironment {
  portable: boolean
  settingsStorePath: string
}

let environmentRequest: Promise<RuntimeEnvironment> | undefined

/** 读取由原生端统一解析的运行模式和存储路径，并在窗口生命周期内复用结果。 */
export function getRuntimeEnvironment(): Promise<RuntimeEnvironment> {
  environmentRequest ??= invoke<RuntimeEnvironment>('get_runtime_environment')
  return environmentRequest
}
