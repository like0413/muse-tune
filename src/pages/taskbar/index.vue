<script setup lang="ts">
import { useElementBounding, useElementHover, useMutationObserver } from '@vueuse/core'
import type { CSSProperties } from 'vue'

import { useLyrics } from '@/features/lyrics/useLyrics'
import { useTaskbarLyricsSettings } from '@/features/lyrics/useTaskbarLyricsSettings'
import { useMediaProgress } from '@/features/media/useMediaProgress'
import { useMediaSession } from '@/features/media/useMediaSession'
import { useMediaSessionSelectionPolicy } from '@/features/media/useMediaSessionSelectionPolicy'
import { TASKBAR_WIDTH_PRESETS } from '@/features/settings/bar-width'
import { isTaskbarCoverVisibleInMode } from '@/features/settings/cover'
import { type TaskbarElement } from '@/features/settings/element-order'
import { useTaskbarAutoHide } from '@/features/taskbar/useTaskbarAutoHide'
import { useTaskbarCoverAppearance } from '@/features/taskbar/useTaskbarCoverAppearance'
import { useTaskbarNativeMenu } from '@/features/taskbar/useTaskbarNativeMenu'
import { useTaskbarViewSettings } from '@/features/taskbar/useTaskbarViewSettings'
import { useTaskbarForegroundColor } from '@/features/theme/useTaskbarForegroundColor'
import { useTaskbarProgressColor } from '@/features/theme/useTaskbarProgressColor'
import { useAutomaticUpdateMonitor } from '@/features/updater/useAutomaticUpdateMonitor'

import AudioSpectrumElement from './components/AudioSpectrumElement.vue'
import CoverElement from './components/CoverElement.vue'
import LyricsNoticeElement from './components/lyrics/LyricsNoticeElement.vue'
import LyricsElement from './components/LyricsElement.vue'
import PlaybackControlsElement from './components/PlaybackControlsElement.vue'
import TrackInfoElement from './components/TrackInfoElement.vue'

const { t } = useI18n({ useScope: 'global' })

const { session: mediaSession, timeline, controlPending, control } = useMediaSession()
const playbackStatus = computed(() => mediaSession.value?.playback.status ?? 'unknown')
const { positionMs, progress } = useMediaProgress(timeline, playbackStatus)
const { settings: lyricsSettings } = useTaskbarLyricsSettings()
// 歌词时间轴始终以歌曲起点为零，GSMTC 对片段媒体可能提供非零起点。
const lyricsPositionMs = computed(() =>
  Math.max(
    0,
    positionMs.value - (timeline.value?.startTimeMs ?? 0) - lyricsSettings.value.timingOffsetMs,
  ),
)
const { lyrics } = useLyrics()
const { appearance: coverAppearance } = useTaskbarCoverAppearance()
const taskbarRoot = useTemplateRef<HTMLElement>('taskbarRoot')
const contentRoot = useTemplateRef<HTMLElement>('contentRoot')
const normalLayer = useTemplateRef<HTMLElement>('normalLayer')
const normalCoverAnchor = useTemplateRef<HTMLElement>('normalCoverAnchor')
const lyricsCoverAnchor = useTemplateRef<HTMLElement>('lyricsCoverAnchor')
const isTaskbarHovered = useElementHover(taskbarRoot)
const { backgroundTransparency, progressStyle, progressPosition, elementOrder, taskbarWidth } =
  useTaskbarViewSettings()
const isCompact = computed(() => taskbarWidth.value <= TASKBAR_WIDTH_PRESETS.compact)
// 所有解析入口都要求有效播放器时间线；纯音乐结论本身不伪装成歌词行。
const hasReliableLyricsTimeline = computed(() => timeline.value !== null)
const hasLyricsContent = computed(
  () =>
    (lyrics.value.status === 'ready' && lyrics.value.lines.length > 0) ||
    lyrics.value.status === 'instrumental',
)
const showLyrics = computed(
  () =>
    lyricsSettings.value.enabled &&
    playbackStatus.value === 'playing' &&
    hasReliableLyricsTimeline.value &&
    hasLyricsContent.value &&
    !isTaskbarHovered.value,
)
const normalCoverVisible = computed(() =>
  isTaskbarCoverVisibleInMode(coverAppearance.value.visibility, 'normal'),
)
const lyricsCoverVisible = computed(() =>
  isTaskbarCoverVisibleInMode(coverAppearance.value.visibility, 'lyrics'),
)
const activeCoverVisible = computed(() =>
  showLyrics.value ? lyricsCoverVisible.value : normalCoverVisible.value,
)
const contentBounds = useElementBounding(contentRoot)
const normalCoverBounds = useElementBounding(normalCoverAnchor)
const lyricsCoverBounds = useElementBounding(lyricsCoverAnchor)
useMediaSessionSelectionPolicy()
useTaskbarAutoHide(mediaSession)
useAutomaticUpdateMonitor()
const { show: showNativeMenu } = useTaskbarNativeMenu()
const thumbnailDataUrl = computed(() => mediaSession.value?.metadata.thumbnailDataUrl ?? null)
const { progressColor } = useTaskbarProgressColor(thumbnailDataUrl)

const { foregroundColor } = useTaskbarForegroundColor(backgroundTransparency)

/** 普通层始终保持最终排列；封面位置由同尺寸锚点预留。 */
const normalElementStyle = computed<Record<TaskbarElement, CSSProperties>>(() => ({
  cover: { order: elementOrder.value.indexOf('cover') },
  'track-info': { order: elementOrder.value.indexOf('track-info') },
  controls: { order: elementOrder.value.indexOf('controls') },
}))

/** 把唯一的真实封面移动到当前模式的锚点，两个内容层中不会产生封面副本。 */
const coverMotionStyle = computed<CSSProperties>(() => {
  const target = showLyrics.value ? lyricsCoverBounds : normalCoverBounds
  const ready = activeCoverVisible.value && contentBounds.width.value > 0 && target.width.value > 0
  if (!ready) return { opacity: 0 }

  return {
    opacity: 1,
    transform: `translate3d(${target.left.value - contentBounds.left.value}px, ${target.top.value - contentBounds.top.value}px, 0)`,
  }
})

/** 在排列、控件可见性或窗口尺寸变化后刷新两个封面锚点。 */
function refreshCoverAnchors() {
  void nextTick(() => {
    contentBounds.update()
    normalCoverBounds.update()
    lyricsCoverBounds.update()
  })
}

useMutationObserver(normalLayer, refreshCoverAnchors, { childList: true, subtree: true })
watch([elementOrder, () => coverAppearance.value.visibility], refreshCoverAnchors)

/** 仅改变页面背景 Alpha，高透明时由前景色策略保证内容对比度。 */
const backgroundStyle = computed(() => ({
  backgroundColor: `color-mix(in srgb, var(--taskbar-background) ${100 - backgroundTransparency.value}%, transparent)`,
  color: foregroundColor.value,
  '--taskbar-secondary-foreground': '#adb1b3',
}))

/** 仅把解析后的主题色暴露给进度条，避免影响全局 primary 色。 */
const progressColorStyle = computed(() => ({
  '--taskbar-progress-color': progressColor.value,
}))

/** 计算条形进度已经播放部分的宽度。 */
const barProgressStyle = computed(() => ({
  width: `${progress.value}%`,
}))

const progressBarPositionClass = computed(() =>
  progressPosition.value === 'top' ? 'top-0' : 'bottom-0',
)

/** 原生菜单关闭后纠正 WebView 可能遗漏 mouseleave 而残留的悬停状态。 */
async function openNativeMenu() {
  await showNativeMenu()
  isTaskbarHovered.value = false
}

/** 菜单关闭时若指针仍在 bar 内，下一次移动立即恢复普通控制层。 */
function restoreTaskbarHover() {
  isTaskbarHovered.value = true
}

/** 用贴近任务栏背景的同色系渐变标示已播放区域，避免与歌词颜色混在一起。 */
const verticalProgressStyle = computed(() => ({
  width: `${progress.value}%`,
  background:
    'linear-gradient(to right, transparent 0%, color-mix(in srgb, var(--taskbar-progress-color) 40%, var(--taskbar-background)) 100%)',
}))

onMounted(refreshCoverAnchors)
</script>

<template>
  <main
    ref="taskbarRoot"
    class="text-taskbar-foreground relative flex size-full items-center gap-2 overflow-hidden px-2 py-1 shadow-sm select-none"
    :style="[backgroundStyle, progressColorStyle]"
    @contextmenu.prevent="openNativeMenu"
    @mousemove="restoreTaskbarHover"
  >
    <AudioSpectrumElement
      :theme-color="progressColor"
      :foreground-color="foregroundColor"
      :progress="progress"
      :overlaps-progress-gradient="progressStyle === 'vertical-gradient'"
    />

    <div ref="contentRoot" class="relative z-10 min-w-0 flex-1 self-stretch">
      <div
        ref="normalLayer"
        class="taskbar-mode-layer"
        :class="showLyrics ? 'pointer-events-none opacity-0' : 'opacity-100'"
        :aria-hidden="showLyrics"
        :inert="showLyrics || undefined"
      >
        <div
          v-if="normalCoverVisible"
          ref="normalCoverAnchor"
          class="size-8 shrink-0"
          :style="normalElementStyle.cover"
          aria-hidden="true"
        />
        <TrackInfoElement :session="mediaSession" :style="normalElementStyle['track-info']" />
        <PlaybackControlsElement
          :session="mediaSession"
          :pending="controlPending"
          :theme-color="progressColor"
          :compact="isCompact"
          :style="normalElementStyle.controls"
          @control="control"
        />
      </div>

      <div
        class="taskbar-mode-layer pointer-events-none"
        :class="showLyrics ? 'opacity-100' : 'opacity-0'"
        :aria-hidden="!showLyrics"
      >
        <div
          v-if="lyricsCoverVisible"
          ref="lyricsCoverAnchor"
          class="size-8 shrink-0"
          aria-hidden="true"
        />
        <LyricsElement
          v-if="lyrics.status === 'ready'"
          :key="lyrics.trackKey ?? 'no-track'"
          :lyrics="lyrics"
          :position-ms="lyricsPositionMs"
          :settings="lyricsSettings"
          :theme-color="progressColor"
        />
        <LyricsNoticeElement
          v-else-if="lyrics.status === 'instrumental'"
          :text="t('taskbar.lyrics.instrumental')"
          :settings="lyricsSettings"
          :theme-color="progressColor"
        />
      </div>

      <div
        v-if="activeCoverVisible"
        class="taskbar-cover-motion pointer-events-none absolute top-0 left-0 z-20 size-8"
        :style="coverMotionStyle"
      >
        <CoverElement :session="mediaSession" :appearance="coverAppearance" />
      </div>
    </div>

    <div
      v-if="timeline"
      class="pointer-events-none absolute inset-0"
      role="progressbar"
      :aria-label="t('media.progress')"
      aria-valuemin="0"
      aria-valuemax="100"
      :aria-valuenow="progress"
    >
      <div
        v-if="progressStyle === 'bottom'"
        class="absolute left-0 z-0 h-0.5 bg-(--taskbar-progress-color)"
        :class="progressBarPositionClass"
        :style="barProgressStyle"
      />
      <div v-else class="absolute inset-y-0 left-0 z-0" :style="verticalProgressStyle" />
    </div>
  </main>
</template>

<style scoped>
.taskbar-mode-layer {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  transition: opacity 280ms ease;
}

.taskbar-cover-motion {
  transition:
    transform 320ms cubic-bezier(0.22, 1, 0.36, 1),
    opacity 120ms ease;
}
</style>
