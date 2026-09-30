import { openUrl } from '@tauri-apps/plugin-opener'
import { toast } from 'vue-sonner'

import { getErrorMessage } from '@/features/feedback/errors'

/** 项目入口共用系统浏览器打开行为与失败提示。 */
export function useProjectLinks() {
  const { t } = useI18n({ useScope: 'global' })

  /** 由 Tauri opener 打开固定的项目地址。 */
  async function openProjectUrl(url: string) {
    try {
      await openUrl(url)
    } catch (error) {
      toast.error(getErrorMessage(error, t('settings.about.project.openFailed')))
    }
  }

  return { openProjectUrl }
}
