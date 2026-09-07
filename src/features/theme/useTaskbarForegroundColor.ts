import type { UnlistenFn } from '@tauri-apps/api/event'
import type { Ref } from 'vue'

import {
  getSystemForegroundColor,
  listenSystemForegroundColorChange,
} from '@/features/system/foreground-color'

const SYSTEM_FOREGROUND_TRANSPARENCY_THRESHOLD = 70

/** 在 bar 高透明时使用 Windows 前景色，否则保留 bar 自身颜色模式。 */
export function useTaskbarForegroundColor(backgroundTransparency: Readonly<Ref<number>>) {
  const systemForegroundColor = shallowRef<string | null>(null)
  let colorRevision = 0
  let disposed = false
  let unlisten: UnlistenFn | undefined

  const foregroundColor = computed(() =>
    backgroundTransparency.value >= SYSTEM_FOREGROUND_TRANSPARENCY_THRESHOLD &&
    systemForegroundColor.value
      ? systemForegroundColor.value
      : 'var(--taskbar-foreground)',
  )

  /** 先建立事件监听再读取初值，避免系统颜色变更竞态。 */
  async function initialize() {
    try {
      const stopListener = await listenSystemForegroundColorChange((color) => {
        colorRevision += 1
        systemForegroundColor.value = color
      })
      if (disposed) {
        stopListener()
        return
      }
      unlisten = stopListener
      const revisionBeforeRead = colorRevision
      const color = await getSystemForegroundColor()
      if (!disposed && colorRevision === revisionBeforeRead) systemForegroundColor.value = color
    } catch (error) {
      console.error('初始化 Windows 前景色失败', error)
    }
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })

  return { foregroundColor }
}
