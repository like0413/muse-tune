import { DEFAULT_APPLICATION_LOCALE } from '@/features/settings/defaults'

export const APPLICATION_LOCALES = ['zh-Hans', 'zh-Hant', 'en'] as const

export type ApplicationLocale = (typeof APPLICATION_LOCALES)[number]

export { DEFAULT_APPLICATION_LOCALE }

/** 返回 Intl 与 HTML lang 使用的标准语言标识。 */
export function getApplicationLocaleTag(locale: unknown): ApplicationLocale {
  return isApplicationLocale(locale) ? locale : DEFAULT_APPLICATION_LOCALE
}

/** 判断外部值是否为应用支持的界面语言。 */
export function isApplicationLocale(value: unknown): value is ApplicationLocale {
  return typeof value === 'string' && APPLICATION_LOCALES.some((locale) => locale === value)
}
