import { LazyStore } from '@tauri-apps/plugin-store'

import { getRuntimeEnvironment } from '@/features/runtime/environment'

let storeRequest: Promise<LazyStore> | undefined

/** 创建指向原生端所选配置目录的 Store；便携版因此不会回落到系统 AppData。 */
function getSettingsStore(): Promise<LazyStore> {
  storeRequest ??= getRuntimeEnvironment().then(
    ({ settingsStorePath }) => new LazyStore(settingsStorePath, { autoSave: 100 }),
  )
  return storeRequest
}

/**
 * 应用设置共用的延迟加载存储。
 * `set` 完成表示 Store 后端已接受新值，磁盘写入由 100ms 自动保存任务调度。
 */
export const settingsStore = {
  /** 读取一个设置值。 */
  async get<T>(key: string): Promise<T | undefined> {
    return (await getSettingsStore()).get<T>(key)
  },
  /** 写入一个设置值；磁盘落盘仍由 Store 的自动保存任务合并。 */
  async set<T>(key: string, value: T): Promise<void> {
    await (await getSettingsStore()).set(key, value)
  },
  /** 立即把当前全部变更刷新到磁盘，供需要在关闭前确认持久化的场景使用。 */
  async save(): Promise<void> {
    await (await getSettingsStore()).save()
  },
}
