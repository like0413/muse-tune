import type { UnlistenFn } from '@tauri-apps/api/event'
import { useMediaQuery } from '@vueuse/core'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import {
  getReducedMotionOverride,
  listenReducedMotionOverrideChange,
} from '@/features/settings/reduced-motion'

const applicationOverride = shallowRef(false)
let consumers = 0
let unlisten: UnlistenFn | undefined

/** 把最终偏好写到根节点，使纯 CSS 动画也能同步响应。 */
function applyDocumentPreference(enabled: boolean) {
  document.documentElement.toggleAttribute('data-reduce-motion', enabled)
}

/** 合并应用覆盖与 Windows/浏览器的系统减少动态偏好。 */
export function useReducedMotionPreference() {
  const systemPreference = useMediaQuery('(prefers-reduced-motion: reduce)')
  const reducedMotion = computed(() => applicationOverride.value || systemPreference.value)
  watch(reducedMotion, applyDocumentPreference, { immediate: true })

  onMounted(async () => {
    consumers += 1
    if (consumers > 1) return
    try {
      unlisten = await listenReducedMotionOverrideChange((enabled) => {
        applicationOverride.value = enabled
      })
      applicationOverride.value = await getReducedMotionOverride()
    } catch (error) {
      reportBackgroundFailure('初始化减少动态效果设置失败', error)
    }
  })

  onUnmounted(() => {
    consumers -= 1
    if (consumers > 0) return
    unlisten?.()
    unlisten = undefined
  })

  return readonly(reducedMotion)
}
