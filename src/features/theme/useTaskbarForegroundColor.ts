import { createColor } from 'colorthief'

import { useEventState } from '@/features/ipc/useEventState'
import {
  getSystemForegroundColor,
  listenSystemForegroundColorChange,
} from '@/features/system/foreground-color'
import { colorMode } from '@/lib/color-mode'

const SYSTEM_FOREGROUND_TRANSPARENCY_THRESHOLD = 70

/** 高透明时只有系统前景的明暗与 bar 自身不同，才切换颜色来源。 */
export function useTaskbarForegroundColor(backgroundTransparency: Readonly<Ref<number>>) {
  const systemForegroundColor = useEventState<string | null>(
    {
      read: getSystemForegroundColor,
      subscribe: listenSystemForegroundColorChange,
      failureMessage: '初始化 Windows 前景色失败',
    },
    null,
  )

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

  return { foregroundColor, isSystemForegroundActive }
}
