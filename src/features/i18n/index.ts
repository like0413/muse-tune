import messages from '@intlify/unplugin-vue-i18n/messages'
import { createI18n } from 'vue-i18n'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import {
  DEFAULT_APPLICATION_LOCALE,
  getApplicationLocaleTag,
  type ApplicationLocale,
} from '@/features/i18n/locales'
import { getApplicationLocale } from '@/features/i18n/settings'

export const i18n = createI18n({
  legacy: false,
  globalInjection: false,
  locale: DEFAULT_APPLICATION_LOCALE,
  fallbackLocale: DEFAULT_APPLICATION_LOCALE,
  messages,
})

/** 同步 vue-i18n 与文档语言属性。 */
export function applyApplicationLocale(locale: ApplicationLocale): void {
  i18n.global.locale.value = locale
  document.documentElement.lang = getApplicationLocaleTag(locale)
}

/** 仅供没有 Vue setup 上下文的模块读取当前语言文案。 */
export function translateGlobal(key: string, named?: Record<string, number | string>): string {
  return named ? i18n.global.t(key, named) : i18n.global.t(key)
}

/** 应用挂载前恢复语言，避免先渲染中文再切换造成闪烁。 */
export async function initializeApplicationLocale(): Promise<void> {
  try {
    applyApplicationLocale(await getApplicationLocale())
  } catch (error) {
    reportBackgroundFailure('读取界面语言失败', error)
    applyApplicationLocale(DEFAULT_APPLICATION_LOCALE)
  }
}
