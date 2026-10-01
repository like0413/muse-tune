import type { MaybeRefOrGetter } from 'vue'

import { reportRepeatedFailure } from '@/features/feedback/errors'

import { convertChineseTexts } from './client'
import type { LyricsChineseVariant } from './types'

/**
 * 在输入或目标字形变化时批量转换短文本；请求序号阻止慢响应覆盖较新的歌曲。
 */
export function useConvertedChineseTexts(
  texts: MaybeRefOrGetter<readonly string[]>,
  chineseVariant: MaybeRefOrGetter<LyricsChineseVariant>,
) {
  const convertedTexts = shallowRef<string[]>([])
  let revision = 0
  let previousTexts: readonly string[] | undefined
  let previousVariant: LyricsChineseVariant | undefined

  watch(
    [() => toValue(texts), () => toValue(chineseVariant)],
    async ([nextTexts, nextVariant]) => {
      // 媒体快照更新不一定改变文字；内容相同就保留结果，避免重复 IPC 和简繁转换。
      if (
        nextVariant === previousVariant &&
        previousTexts?.length === nextTexts.length &&
        nextTexts.every((text, index) => text === previousTexts?.[index])
      )
        return
      previousTexts = [...nextTexts]
      previousVariant = nextVariant
      const currentRevision = ++revision
      const fallback = [...nextTexts]
      convertedTexts.value = fallback
      if (nextVariant === 'original') return

      try {
        const converted = await convertChineseTexts(fallback, nextVariant)
        if (revision === currentRevision) convertedTexts.value = converted
      } catch (error) {
        if (revision === currentRevision) {
          // 失败不缓存，下次同内容快照仍可重试。
          previousTexts = undefined
          reportRepeatedFailure('转换歌曲名与歌手简繁失败', error)
        }
      }
    },
    { immediate: true },
  )
  onScopeDispose(() => revision++)

  return { convertedTexts: readonly(convertedTexts) }
}
