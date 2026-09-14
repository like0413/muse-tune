import { LazyStore } from '@tauri-apps/plugin-store'

const SETTINGS_STORE_PATH = 'settings.json'

/**
 * 应用设置共用的延迟加载存储。
 * `set` 完成表示 Store 后端已接受新值，磁盘写入由 100ms 自动保存任务调度。
 */
export const settingsStore = new LazyStore(SETTINGS_STORE_PATH, { autoSave: 100 })
