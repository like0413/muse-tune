<script setup lang="ts">
const props = defineProps<{
  image: HTMLImageElement | null
}>()

/** 常驻图层数量：新封面写入另一个图层后再切换，靠交叉淡入完成过渡。 */
const SLOT_COUNT = 2

const sources = shallowRef<(string | null)[]>(Array.from({ length: SLOT_COUNT }, () => null))
const activeIndex = shallowRef(0)
let displayedSource: string | null = null

/** 写入指定图层的封面地址；整体替换数组以触发更新。 */
function setSource(index: number, source: string) {
  const next = [...sources.value]
  next[index] = source
  sources.value = next
}

/** 交叉淡入完成后释放非活动图层，避免上一首封面继续占用解码与 GPU 资源。 */
function releaseInactiveSource(index: number, event: TransitionEvent) {
  if (event.propertyName !== 'opacity' || index === activeIndex.value) return
  const next = [...sources.value]
  next[index] = null
  sources.value = next
}

/**
 * 交叉淡入淡出：新封面先写入未激活的常驻图层，绘制完成后再切换激活项。
 * 图层始终存在，仅调整透明度；通过隔离的 plus-lighter 混合保持两层总覆盖率，
 * 避免普通透明度叠加在过渡中途露出更多底色而变暗。
 */
watch(
  () => props.image,
  (image) => {
    const source = image?.src ?? null
    if (source === displayedSource) return
    const wasVisible = displayedSource !== null
    displayedSource = source
    if (!source) return

    // 上一次没有可见封面时直接写入激活图层，避免外层淡入过程中先露出更早的封面。
    if (!wasVisible) {
      setSource(activeIndex.value, source)
      return
    }

    const target = (activeIndex.value + 1) % SLOT_COUNT
    setSource(target, source)
    void nextTick(() => {
      requestAnimationFrame(() => (activeIndex.value = target))
    })
  },
  { immediate: true },
)
</script>

<template>
  <div
    class="cover-background pointer-events-none absolute -inset-6 z-0"
    :style="{ opacity: image ? 0.4 : 0 }"
    aria-hidden="true"
  >
    <div
      v-for="(source, index) in sources"
      :key="index"
      class="cover-image absolute inset-0"
      :style="{ opacity: index === activeIndex && source ? 1 : 0 }"
      @transitionend.self="releaseInactiveSource(index, $event)"
    >
      <div
        v-if="source"
        class="cover-image-blur absolute inset-0"
        :style="{ backgroundImage: `url(${JSON.stringify(source)})` }"
        aria-hidden="true"
      />
    </div>
  </div>
</template>

<style scoped>
/* 常驻图层只切换透明度，并保持独立合成层，避免过渡前后重新栅格化导致的亮度跳变。 */
.cover-image {
  /* 两层透明度相加，避免交叉淡入淡出产生中途变暗。 */
  mix-blend-mode: plus-lighter;
  background-position: center;
  background-size: 100% 100%;
  opacity: 0;
  transition: opacity 420ms ease;
  will-change: opacity;
}

.cover-image-blur {
  background-position: center;
  background-size: 100% 100%;
  filter: blur(20px) saturate(1.3);
}

.cover-background {
  /* 将混合限制在封面背景内，不影响任务栏底色、歌词与按钮。 */
  isolation: isolate;
  transition: opacity 420ms ease;
}
</style>
