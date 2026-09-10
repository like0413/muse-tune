import type { TaskbarLyricsSettings } from '@/features/settings/lyrics'

export interface ResolvedLyricsAppearance {
  playedColor: string
  unplayedColor: string
  fontFamily?: string
}

/** 将字体族名称转为单个安全的 CSS 字体族值。 */
function quoteFontFamily(fontFamily: string): string | undefined {
  if (!fontFamily) return undefined
  return `"${fontFamily.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`
}

/** 解析颜色和字体；空字体不覆写任务栏原有字体。 */
export function resolveTaskbarLyricsAppearance(
  settings: Readonly<TaskbarLyricsSettings>,
  themeColor: string,
): ResolvedLyricsAppearance {
  const usesCustomColors = settings.colorScheme === 'custom'

  return {
    playedColor: usesCustomColors ? settings.playedColor : themeColor,
    unplayedColor: usesCustomColors
      ? settings.unplayedColor
      : 'var(--taskbar-secondary-foreground)',
    fontFamily: quoteFontFamily(settings.fontFamily),
  }
}
