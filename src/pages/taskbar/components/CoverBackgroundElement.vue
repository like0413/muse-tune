<script setup lang="ts">
const props = defineProps<{
  image: HTMLImageElement | null
  flow: boolean
}>()

interface BackgroundArtwork {
  source: string
  order: number
}

const artwork = shallowRef<BackgroundArtwork | null>(null)
let artworkOrder = 0

/** 共享封面已完成解码并处理短暂空值，此处只同步当前图层。 */
watch(
  () => props.image,
  (image) => {
    if (!image) {
      artwork.value = null
      return
    }

    artwork.value = { source: image.src, order: ++artworkOrder }
  },
  { immediate: true },
)
</script>

<template>
  <div
    class="cover-background pointer-events-none absolute -inset-6 z-0"
    :style="{ opacity: artwork ? 0.4 : 0 }"
    aria-hidden="true"
  >
    <Transition name="cover-image" mode="in-out">
      <div
        v-if="artwork"
        :key="artwork.source"
        class="cover-image absolute inset-0"
        :class="{ 'cover-image-flow': flow }"
        :style="{
          backgroundImage: `url(${JSON.stringify(artwork.source)})`,
          zIndex: artwork.order,
        }"
      />
    </Transition>
  </div>
</template>

<style scoped>
.cover-image {
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
  will-change: transform;
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

.cover-image-enter-active {
  transition: opacity 420ms ease;
}

.cover-image-enter-from {
  opacity: 0;
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
