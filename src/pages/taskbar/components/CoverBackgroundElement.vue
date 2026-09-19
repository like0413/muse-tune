<script setup lang="ts">
const props = defineProps<{
  image: HTMLImageElement | null
  flow: boolean
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

/**
 * 交叉淡入淡出：新封面先写入未激活的常驻图层，绘制完成后再切换激活项。
 * 图层始终存在，只有透明度在变，因此过渡中不会出现图层创建或回收带来的亮度跳变，
 * 旧封面也会在过渡期间保持可见，不会整条 bar 先暗下去。
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
    >
      <div
        class="cover-image-blur absolute inset-0"
        :style="source ? { backgroundImage: `url(${JSON.stringify(source)})` } : undefined"
        aria-hidden="true"
      />
      <div
        v-if="flow"
        class="cover-image-flow absolute inset-0"
        :style="source ? { backgroundImage: `url(${JSON.stringify(source)})` } : undefined"
        aria-hidden="true"
      />
    </div>
  </div>
</template>

<style scoped>
/* 常驻图层只切换透明度，并保持独立合成层，避免过渡前后重新栅格化导致的亮度跳变。 */
.cover-image {
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

/* 放大同一封面的局部颜色，再沿不同闭合路径移动，避免原路折返。 */
.cover-image-flow::before,
.cover-image-flow::after {
  content: '';
  position: absolute;
  inset: -15%;
  background-image: inherit;
  background-repeat: no-repeat;
  filter: saturate(1.5);
}

.cover-image-flow::before {
  background-position: 20% 45%;
  background-size: 145% 145%;
  mask-image: radial-gradient(ellipse 55% 75% at 30% 40%, black 15%, transparent 85%);
  animation: cover-color-drift-a 11s linear infinite;
}

.cover-image-flow::after {
  background-position: 80% 55%;
  background-size: 135% 155%;
  mask-image: radial-gradient(ellipse 55% 75% at 70% 60%, black 15%, transparent 85%);
  animation: cover-color-drift-b 17s linear infinite;
}

.cover-background {
  transition: opacity 420ms ease;
}

@keyframes cover-color-drift-a {
  0%,
  100% {
    transform: translate3d(-22%, -13%, 0) scale(1.04);
  }

  18% {
    transform: translate3d(12%, -20%, 0) scale(1.13);
  }

  39% {
    transform: translate3d(26%, 10%, 0) scale(1.08);
  }

  63% {
    transform: translate3d(2%, 20%, 0) scale(1.16);
  }

  82% {
    transform: translate3d(-26%, 8%, 0) scale(1.09);
  }
}

@keyframes cover-color-drift-b {
  0%,
  100% {
    transform: translate3d(24%, 12%, 0) scale(1.12);
  }

  22% {
    transform: translate3d(-8%, 22%, 0) scale(1.05);
  }

  46% {
    transform: translate3d(-28%, -5%, 0) scale(1.14);
  }

  71% {
    transform: translate3d(-4%, -20%, 0) scale(1.07);
  }

  86% {
    transform: translate3d(22%, -12%, 0) scale(1.15);
  }
}
</style>
