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

  watch(
    [() => toValue(texts), () => toValue(chineseVariant)],
    async ([nextTexts, nextVariant]) => {
      const currentRevision = ++revision
      const fallback = [...nextTexts]
      convertedTexts.value = fallback
      if (nextVariant === 'original') return

      try {
        const converted = await convertChineseTexts(fallback, nextVariant)
        if (revision === currentRevision) convertedTexts.value = converted
      } catch (error) {
        if (revision === currentRevision) {
          reportRepeatedFailure('转换歌曲名与歌手简繁失败', error)
        }
      }
    },
    { immediate: true },
  )
  onScopeDispose(() => revision++)

  return { convertedTexts: readonly(convertedTexts) }
}
