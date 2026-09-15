import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from '@/features/settings/store'

import { DEFAULT_APPLICATION_LOCALE, isApplicationLocale, type ApplicationLocale } from './locales'

const APPLICATION_LOCALE_KEY = 'application.locale'
const APPLICATION_LOCALE_CHANGED_EVENT = 'settings://application-locale-changed'

/** 读取持久化界面语言，缺失或损坏时回退为简体中文。 */
export async function getApplicationLocale(): Promise<ApplicationLocale> {
  const locale = await settingsStore.get<unknown>(APPLICATION_LOCALE_KEY)
  return isApplicationLocale(locale) ? locale : DEFAULT_APPLICATION_LOCALE
}

/** 保存界面语言，并通知其他 WebView 同步切换。 */
export async function setApplicationLocale(locale: ApplicationLocale): Promise<void> {
  await settingsStore.set(APPLICATION_LOCALE_KEY, locale)
  await emit(APPLICATION_LOCALE_CHANGED_EVENT, locale)
}

/** 监听其他 WebView 发布的界面语言变化。 */
export async function listenApplicationLocaleChange(
  handler: (locale: ApplicationLocale) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(APPLICATION_LOCALE_CHANGED_EVENT, ({ payload }) => {
    if (isApplicationLocale(payload)) handler(payload)
  })
}
