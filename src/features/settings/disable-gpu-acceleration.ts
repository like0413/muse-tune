import { invoke } from '@tauri-apps/api/core'

import { DEFAULT_DISABLE_GPU_ACCELERATION } from './defaults'
import { settingsStore } from './store'

const DISABLE_GPU_ACCELERATION_KEY = 'app.disableGpuAcceleration'

/** 读取持久化的禁用 GPU 加速覆盖项。 */
export async function getDisableGpuAcceleration(): Promise<boolean> {
  const stored = await settingsStore.get<unknown>(DISABLE_GPU_ACCELERATION_KEY)
  return typeof stored === 'boolean' ? stored : DEFAULT_DISABLE_GPU_ACCELERATION
}

/**
 * 保存禁用 GPU 加速开关。
 *
 * 该值在原生端于启动时捕获，供所有 WebView 统一使用；本次会话仍沿用旧的启动值，需重启后生效。
 * 这里在 `set` 后强制 `save()`，确保即使随后被立即结束进程，磁盘上也已落盘该值。
 */
export async function setDisableGpuAcceleration(enabled: boolean): Promise<void> {
  await settingsStore.set(DISABLE_GPU_ACCELERATION_KEY, enabled)
  await settingsStore.save()
}

/** 返回当前会话实际生效的禁用 GPU 加速状态（启动时捕获，重启前保持稳定）。 */
export async function getEffectiveDisableGpuAcceleration(): Promise<boolean> {
  return invoke<boolean>('get_gpu_acceleration_setting')
}
