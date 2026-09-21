import type { UnlistenFn } from '@tauri-apps/api/event'
import { createColor } from 'colorthief'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import {
  getSystemForegroundColor,
  listenSystemForegroundColorChange,
} from '@/features/system/foreground-color'
import { colorMode } from '@/lib/color-mode'

const SYSTEM_FOREGROUND_TRANSPARENCY_THRESHOLD = 70

/** 高透明时只有系统前景的明暗与 bar 自身不同，才切换颜色来源。 */
export function useTaskbarForegroundColor(backgroundTransparency: Readonly<Ref<number>>) {
  const systemForegroundColor = shallowRef<string | null>(null)
  let colorRevision = 0
  let disposed = false
  let unlisten: UnlistenFn | undefined

  const systemForegroundIsLight = computed(() => {
    const value = systemForegroundColor.value
    if (!value || !/^#[\da-f]{6}$/i.test(value)) return null
    const channels = Number.parseInt(value.slice(1), 16)
    return createColor((channels >> 16) & 0xff, (channels >> 8) & 0xff, channels & 0xff, 0).isLight
  })
  const isSystemForegroundActive = computed(
    () =>
      backgroundTransparency.value >= SYSTEM_FOREGROUND_TRANSPARENCY_THRESHOLD &&
      systemForegroundIsLight.value !== null &&
      systemForegroundIsLight.value !== (colorMode.value === 'dark'),
  )
  const foregroundColor = computed(() =>
    isSystemForegroundActive.value
      ? (systemForegroundColor.value ?? 'var(--taskbar-foreground)')
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
      reportBackgroundFailure('初始化 Windows 前景色失败', error)
    }
  }

  onMounted(initialize)
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })

  return { foregroundColor, isSystemForegroundActive }
}
