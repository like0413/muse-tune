import { reportBackgroundFailure } from '@/features/feedback/errors'
import { useEventState } from '@/features/ipc/useEventState'
import {
  DEFAULT_TASKBAR_THEME_COLOR,
  getTaskbarThemeColor,
  listenTaskbarThemeColorChange,
  type TaskbarThemeColor,
} from '@/features/settings/theme-color'
import { getSystemAccentColor, listenSystemAccentColorChange } from '@/features/system/accent-color'

import { DEFAULT_BRAND_COLOR_HEX } from './colors'
import { extractTaskbarCoverColor } from './cover-color'

/** 主色提取结果缓存上限；键为曲目身份字符串，避免长期驻留 Data URL 本身。 */
const COVER_COLOR_CACHE_LIMIT = 16

/**
 * 按设置来源解析进度条颜色，并只在封面变化时重新取色。
 * 同一曲目的提取结果会被复用，避免来回切歌时重复解码与像素遍历。
 */
export function useTaskbarProgressColor(
  thumbnailDataUrl: ComputedRef<string | null>,
  trackIdentity: Readonly<Ref<string | null>> = shallowRef(null),
) {
  const setting = useEventState<TaskbarThemeColor>(
    {
      read: getTaskbarThemeColor,
      subscribe: listenTaskbarThemeColorChange,
      failureMessage: '初始化任务栏主题色失败',
    },
    { ...DEFAULT_TASKBAR_THEME_COLOR },
  )
  const systemColor = useEventState(
    {
      read: getSystemAccentColor,
      subscribe: listenSystemAccentColorChange,
      failureMessage: '初始化 Windows 强调色失败',
    },
    DEFAULT_BRAND_COLOR_HEX,
  )
  const coverColor = shallowRef<string | null>(null)
  const extractedColors = new Map<string, string>()
  let extractionRequestId = 0

  /** 记录已提取的主色，并按插入序淘汰最旧的一条。 */
  function rememberCoverColor(identity: string, color: string) {
    if (extractedColors.size >= COVER_COLOR_CACHE_LIMIT) {
      const oldest = extractedColors.keys().next()
      if (!oldest.done) extractedColors.delete(oldest.value)
    }
    extractedColors.set(identity, color)
  }

  /** 异步提取封面主色；新颜色产出前保留上一个有效结果，避免切歌闪色。 */
  async function extractCoverColor() {
    const thumbnail = thumbnailDataUrl.value
    const requestId = ++extractionRequestId
    if (setting.value.source !== 'cover' || !thumbnail) return

    const identity = trackIdentity.value
    const cached = identity === null ? undefined : extractedColors.get(identity)
    if (cached !== undefined) {
      coverColor.value = cached
      return
    }

    try {
      const extractedColor = await extractTaskbarCoverColor(thumbnail)
      if (requestId !== extractionRequestId) return
      coverColor.value = extractedColor
      if (identity !== null) rememberCoverColor(identity, extractedColor)
    } catch (error) {
      if (requestId === extractionRequestId) {
        reportBackgroundFailure('提取封面主色失败', error)
      }
    }
  }

  const progressColor = computed(() => {
    switch (setting.value.source) {
      case 'cover':
        return coverColor.value ?? systemColor.value
      case 'custom':
        return setting.value.customColor
      default:
        return systemColor.value
    }
  })

  // 主题色初值读取与变更事件都要重新取色，统一由这里触发。
  // 合并同一轮曲目信息与来源变化；自定义颜色变化不触发封面解码。
  watch(
    [() => setting.value.source, thumbnailDataUrl, trackIdentity],
    () => void extractCoverColor(),
  )
  onUnmounted(() => {
    // 让仍在进行的取色请求作废，避免卸载后再写回结果。
    extractionRequestId += 1
  })

  return { progressColor }
}
