import { LazyStore } from '@tauri-apps/plugin-store'

const SETTINGS_STORE_PATH = 'settings.json'

/** 应用设置共用的延迟加载存储，避免各设置能力重复创建实例。 */
export const settingsStore = new LazyStore(SETTINGS_STORE_PATH, { autoSave: 100 })
