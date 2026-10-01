import { useMutationObserver, useRafFn, useResizeObserver } from '@vueuse/core'
import type { Ref, WatchSource } from 'vue'

interface CoverPosition {
  x: number
  y: number
}

interface CoverPositionOptions {
  content: Readonly<Ref<HTMLElement | null>>
  layers: readonly Readonly<Ref<HTMLElement | null>>[]
  anchor: Readonly<Ref<HTMLElement | null>>
  enabled: Readonly<Ref<boolean>>
  layout: WatchSource<unknown>
}

/** 按布局事件测量活动封面锚点；同帧合并读取，播放时不监听文本子树变化。 */
export function useTaskbarCoverPosition(options: CoverPositionOptions) {
  const position = shallowRef<CoverPosition | null>(null)
  const resizeTargets = shallowRef<HTMLElement[]>([])

  /** 一次只读取内容容器与活动锚点，并保持未变化的位置对象稳定。 */
  function measurePosition() {
    pause()
    const content = options.content.value
    const anchor = options.anchor.value
    if (!options.enabled.value || !content || !anchor) {
      position.value = null
      return
    }
    const contentBounds = content.getBoundingClientRect()
    const anchorBounds = anchor.getBoundingClientRect()
    if (contentBounds.width <= 0 || anchorBounds.width <= 0) {
      position.value = null
      return
    }
    const x = anchorBounds.left - contentBounds.left
    const y = anchorBounds.top - contentBounds.top
    if (position.value?.x !== x || position.value.y !== y) position.value = { x, y }
  }

  const { pause, resume } = useRafFn(measurePosition, { immediate: false })

  /** 布局已经由 Vue 提交后预约下一帧；多个观察器通知只保留一次读取。 */
  function scheduleMeasurement() {
    if (options.enabled.value) resume()
  }

  /** 控件宽度变化会移动锚点；只观察层与直接子元素，无需跟踪逐字 DOM。 */
  function refreshResizeTargets() {
    const targets = new Set<HTMLElement>()
    if (options.content.value) targets.add(options.content.value)
    for (const layer of options.layers) {
      if (!layer.value) continue
      targets.add(layer.value)
      for (const child of layer.value.children) {
        if (child instanceof HTMLElement) targets.add(child)
      }
    }
    resizeTargets.value = [...targets]
    scheduleMeasurement()
  }

  useResizeObserver(resizeTargets, scheduleMeasurement, { box: 'border-box' })
  useMutationObserver(() => options.layers.map((layer) => layer.value), refreshResizeTargets, {
    childList: true,
  })
  watch([options.content, ...options.layers], refreshResizeTargets, { flush: 'post' })
  watch(
    [options.anchor, options.layout, options.enabled],
    () => {
      if (options.enabled.value) scheduleMeasurement()
      else {
        pause()
        position.value = null
      }
    },
    { flush: 'post' },
  )
  onMounted(refreshResizeTargets)

  return readonly(position)
}
