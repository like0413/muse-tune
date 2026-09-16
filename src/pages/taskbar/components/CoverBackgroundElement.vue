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

/* 流动层取自同一封面，局部色块独立位移；不重新取色，也不改动切歌淡入。 */
.cover-image-flow::before,
.cover-image-flow::after {
  content: '';
  position: absolute;
  inset: -6%;
  background-image: inherit;
  background-position: center;
  background-size: 100% 100%;
  will-change: transform;
}

.cover-image-flow::before {
  mask-image: radial-gradient(ellipse 48% 68% at 28% 38%, black 18%, transparent 100%);
  animation: cover-color-drift-a 3s ease-in-out infinite alternate;
}

.cover-image-flow::after {
  mask-image: radial-gradient(ellipse 48% 68% at 72% 62%, black 18%, transparent 100%);
  animation: cover-color-drift-b 5s ease-in-out infinite alternate;
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
  from {
    transform: translate3d(-9%, -8%, 0) scale(1.08);
  }

  to {
    transform: translate3d(10%, 8%, 0) scale(1.08);
  }
}

@keyframes cover-color-drift-b {
  from {
    transform: translate3d(9%, -7%, 0) scale(1.08);
  }

  to {
    transform: translate3d(-10%, 9%, 0) scale(1.08);
  }
}
</style>
