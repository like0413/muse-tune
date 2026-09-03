<script setup lang="ts">
import { Pause, Play, SkipBack, SkipForward } from '@lucide/vue'
import { shallowRef } from 'vue'

const isPlaying = shallowRef(true)
</script>

<template>
  <main class="taskbar-player" aria-label="Muse Tune 任务栏播放器">
    <div class="cover" aria-hidden="true">
      <span>♪</span>
    </div>

    <div class="track">
      <div class="track-line">
        <span class="title">Muse Tune</span>
        <span class="artist">Muse Tune</span>
      </div>
      <div class="progress" aria-label="播放进度 42%">
        <span class="progress-value" />
      </div>
    </div>

    <div class="controls" aria-label="播放控制">
      <button class="control-button" type="button" title="上一曲" aria-label="上一曲">
        <SkipBack :size="15" :stroke-width="2" />
      </button>
      <button
        class="control-button"
        type="button"
        title="播放或暂停"
        aria-label="播放或暂停"
        @click="isPlaying = !isPlaying"
      >
        <Pause v-if="isPlaying" :size="15" :stroke-width="2.2" />
        <Play v-else :size="15" :stroke-width="2.2" />
      </button>
      <button class="control-button" type="button" title="下一曲" aria-label="下一曲">
        <SkipForward :size="15" :stroke-width="2" />
      </button>
    </div>
  </main>
</template>

<style scoped>
.taskbar-player {
  box-sizing: border-box;
  display: flex;
  width: 100vw;
  height: 100vh;
  align-items: center;
  gap: 8px;
  overflow: hidden;
  padding: 4px 8px;
  color: light-dark(#171717, #f5f5f5);
  background: light-dark(rgb(243 243 243 / 82%), rgb(32 32 32 / 82%));
  border: 1px solid light-dark(rgb(255 255 255 / 50%), rgb(255 255 255 / 9%));
  border-radius: 8px;
  box-shadow: 0 1px 3px rgb(0 0 0 / 18%);
  backdrop-filter: blur(20px) saturate(1.35);
  user-select: none;
}

.cover {
  display: grid;
  width: 32px;
  height: 32px;
  flex: 0 0 32px;
  place-items: center;
  overflow: hidden;
  color: #fff;
  background: linear-gradient(145deg, #8b5cf6, #2563eb);
  border-radius: 5px;
  font-size: 17px;
}

.track {
  min-width: 0;
  flex: 1;
}

.track-line {
  display: flex;
  align-items: baseline;
  gap: 6px;
  white-space: nowrap;
}

.title,
.artist {
  overflow: hidden;
  text-overflow: ellipsis;
}

.title {
  flex: 0 1 auto;
  font-size: 12px;
  font-weight: 600;
}

.artist {
  flex: 1 1 auto;
  color: light-dark(rgb(23 23 23 / 60%), rgb(245 245 245 / 60%));
  font-size: 10px;
}

.progress {
  height: 2px;
  margin-top: 5px;
  overflow: hidden;
  background: light-dark(rgb(0 0 0 / 14%), rgb(255 255 255 / 18%));
  border-radius: 999px;
}

.progress-value {
  display: block;
  width: 42%;
  height: 100%;
  background: currentColor;
  border-radius: inherit;
}

.controls {
  display: flex;
  flex: 0 0 auto;
  gap: 1px;
}

.control-button {
  display: grid;
  width: 28px;
  height: 32px;
  padding: 0;
  place-items: center;
  color: inherit;
  background: transparent;
  border: 0;
  border-radius: 5px;
  font: inherit;
}

.control-button:hover {
  background: light-dark(rgb(0 0 0 / 7%), rgb(255 255 255 / 9%));
}

.control-button:active {
  transform: scale(0.95);
  background: light-dark(rgb(0 0 0 / 11%), rgb(255 255 255 / 13%));
}
</style>
