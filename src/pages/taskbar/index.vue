<script setup lang="ts">
import {
  useElementBounding,
  useElementHover,
  useMutationObserver,
  useThrottleFn,
} from '@vueuse/core'
import type { CSSProperties, VNodeRef } from 'vue'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import { useLyrics } from '@/features/lyrics/useLyrics'
import { useTaskbarLyricsSettings } from '@/features/lyrics/useTaskbarLyricsSettings'
import { toggleCurrentMediaPlayer } from '@/features/media/client'
import { useMediaProgress } from '@/features/media/useMediaProgress'
import { useMediaSession } from '@/features/media/useMediaSession'
import { useMediaSessionSelectionPolicy } from '@/features/media/useMediaSessionSelectionPolicy'
import { useReducedMotionPreference } from '@/features/motion/useReducedMotionPreference'
import { useTaskbarAudioSpectrumSettings } from '@/features/settings/audio-spectrum'
import { TASKBAR_WIDTH_PRESETS } from '@/features/settings/bar-width'
import { isTaskbarCoverVisibleInMode } from '@/features/settings/cover'
import { normalizeTaskbarElementOrder } from '@/features/settings/element-order'
import { useTaskbarAutoHide } from '@/features/taskbar/useTaskbarAutoHide'
import { useTaskbarCoverAppearance } from '@/features/taskbar/useTaskbarCoverAppearance'
import { useTaskbarDisplayedThumbnail } from '@/features/taskbar/useTaskbarDisplayedThumbnail'
import { useTaskbarTrayMenu } from '@/features/taskbar/useTaskbarTrayMenu'
import { useTaskbarViewSettings } from '@/features/taskbar/useTaskbarViewSettings'
import { useTaskbarForegroundColor } from '@/features/theme/useTaskbarForegroundColor'
import { useTaskbarProgressColor } from '@/features/theme/useTaskbarProgressColor'
import { useAutomaticUpdateMonitor } from '@/features/updater/useAutomaticUpdateMonitor'

import AudioSpectrumElement from './components/AudioSpectrumElement.vue'
import CoverBackgroundElement from './components/CoverBackgroundElement.vue'
import CoverElement from './components/CoverElement.vue'
import LyricsNoticeElement from './components/lyrics/LyricsNoticeElement.vue'
import LyricsElement from './components/LyricsElement.vue'
import PlaybackControlsElement from './components/PlaybackControlsElement.vue'
import TrackInfoElement from './components/TrackInfoElement.vue'

const { t } = useI18n({ useScope: 'global' })
const reducedMotion = useReducedMotionPreference()

const { session: mediaSession, timeline, controlPending, control } = useMediaSession()
const playbackStatus = computed(() => mediaSession.value?.playback.status ?? 'unknown')
const { settings: lyricsSettings } = useTaskbarLyricsSettings()
const { lyrics } = useLyrics(computed(() => lyricsSettings.value.enabled))
const { settings: spectrumSettings, ready: spectrumSettingsReady } =
  useTaskbarAudioSpectrumSettings()
const { appearance: coverAppearance } = useTaskbarCoverAppearance()
const taskbarRoot = useTemplateRef<HTMLElement>('taskbarRoot')
const contentRoot = useTemplateRef<HTMLElement>('contentRoot')
const normalLayer = useTemplateRef<HTMLElement>('normalLayer')
const normalCoverAnchor = shallowRef<HTMLElement | null>(null)
const lyricsCoverAnchor = shallowRef<HTMLElement | null>(null)

/** v-for 内的封面锚点仍保持单元素引用，避免模板 ref 被收集成数组。 */
const setNormalCoverAnchor: VNodeRef = (element) => {
  normalCoverAnchor.value = element instanceof HTMLElement ? element : null
}

/** 歌词层同样只有一个封面锚点。 */
const setLyricsCoverAnchor: VNodeRef = (element) => {
  lyricsCoverAnchor.value = element instanceof HTMLElement ? element : null
}
const isTaskbarHovered = useElementHover(taskbarRoot)
const {
  backgroundTransparency,
  backgroundStyle: backgroundMode,
  progressStyle,
  progressVisible,
  progressPosition,
  elementOrder,
} = useTaskbarViewSettings()
const { visible: taskbarContentVisible } = useTaskbarAutoHide(mediaSession)
// 必须测量根元素（视口）而不是 bar 内容容器：内容容器会被内容撑大（紧凑模式本身会改变内容宽度），
// 用测量结果判断会形成死循环；同时 bar 窗口由原生 SetWindowPos 改尺寸，不一定派发 window resize 事件。
const { width: barWindowWidth } = useElementBounding(document.documentElement, {
  windowScroll: false,
})
// 宽度可能来自固定设置或自适应计算，因此按实际窗口宽度判断紧凑模式。
// 窗口按 DPI 换算物理像素、WebView 再折回 CSS 像素时会有几 px 取整误差，需要容差。
const COMPACT_WIDTH_TOLERANCE = 2
const isCompact = computed(
  () =>
    barWindowWidth.value > 0 &&
    barWindowWidth.value <= TASKBAR_WIDTH_PRESETS.compact + COMPACT_WIDTH_TOLERANCE,
)
/** 歌词状态就绪且行非空；不含设置开关、播放状态与悬停等进入歌词模式的条件。 */
const hasTimedLyrics = computed(
  () => lyrics.value.status === 'ready' && lyrics.value.lines.length > 0,
)
// 平台已给出结论、不需要歌词行的状态：纯音乐与“没有歌词”。
const hasNoticeOnly = computed(
  () => lyrics.value.status === 'instrumental' || lyrics.value.status === 'no_lyrics',
)
/** 结论型状态的提示文案；与 `hasNoticeOnly` 覆盖的状态一一对应。 */
const lyricsNoticeText = computed(() =>
  lyrics.value.status === 'no_lyrics'
    ? t('taskbar.lyrics.noLyrics')
    : t('taskbar.lyrics.instrumental'),
)
const hasLyricsContent = computed(() => hasTimedLyrics.value || hasNoticeOnly.value)
/** 歌词是否需要持续推进时间轴；与刷新是否平滑无关。 */
const needsLyricsTimeline = computed(
  () =>
    lyricsSettings.value.enabled &&
    hasTimedLyrics.value &&
    playbackStatus.value === 'playing' &&
    !isTaskbarHovered.value,
)
// 开启减少动态效果时降级为 1 FPS 低频更新，避免逐帧重算歌词并触发重绘。
const needsSmoothProgress = computed(() => needsLyricsTimeline.value && !reducedMotion.value)
const needsProgress = computed(
  () =>
    taskbarContentVisible.value &&
    (progressVisible.value ||
      (spectrumSettings.value.visible && progressStyle.value === 'vertical-gradient') ||
      needsLyricsTimeline.value),
)
const { positionMs, progress } = useMediaProgress(
  timeline,
  playbackStatus,
  needsProgress,
  needsSmoothProgress,
)
// 歌词时间轴始终以歌曲起点为零，GSMTC 对片段媒体可能提供非零起点。
const lyricsPositionMs = computed(() =>
  Math.max(
    0,
    positionMs.value - (timeline.value?.startTimeMs ?? 0) - lyricsSettings.value.timingOffsetMs,
  ),
)
// 所有解析入口都要求有效播放器时间线；纯音乐结论本身不伪装成歌词行。
const hasReliableLyricsTimeline = computed(() => timeline.value !== null)
const showLyrics = computed(
  () =>
    lyricsSettings.value.enabled &&
    playbackStatus.value === 'playing' &&
    hasReliableLyricsTimeline.value &&
    hasLyricsContent.value &&
    !isTaskbarHovered.value,
)
/** 只缩短歌词层，普通层始终保持完整宽度；0.5rem 与元素间距保持一致。 */
const lyricsLayerStyle = computed<CSSProperties>(() => ({
  right:
    spectrumSettingsReady.value && spectrumSettings.value.visible
      ? `calc(${spectrumSettings.value.width}px + 0.5rem)`
      : 0,
}))
const normalCoverVisible = computed(() =>
  isTaskbarCoverVisibleInMode(coverAppearance.value.visibility, 'normal'),
)
const lyricsCoverVisible = computed(() =>
  isTaskbarCoverVisibleInMode(coverAppearance.value.visibility, 'lyrics'),
)
const activeCoverVisible = computed(() =>
  showLyrics.value ? lyricsCoverVisible.value : normalCoverVisible.value,
)
// 锚点位于固定尺寸的 bar 窗口内，不随页面滚动变化，无需订阅全局 scroll。
const contentBounds = useElementBounding(contentRoot, { windowScroll: false })
const normalCoverBounds = useElementBounding(normalCoverAnchor, { windowScroll: false })
const lyricsCoverBounds = useElementBounding(lyricsCoverAnchor, { windowScroll: false })
useMediaSessionSelectionPolicy()
useAutomaticUpdateMonitor()
useTaskbarTrayMenu(() => ({
  normalCover: isTaskbarCoverVisibleInMode(coverAppearance.value.visibility, 'normal'),
  lyricsCover: isTaskbarCoverVisibleInMode(coverAppearance.value.visibility, 'lyrics'),
  lyrics: lyricsSettings.value.enabled,
  spectrum: spectrumSettings.value.visible,
}))
const thumbnailDataUrl = computed(() => mediaSession.value?.metadata.thumbnailDataUrl ?? null)
/** 曲目身份键；用于复用封面主色提取结果，避免来回切歌时重复解码与像素遍历。 */
const trackIdentity = computed(() => {
  const metadata = mediaSession.value?.metadata
  if (!metadata) return null
  return `${metadata.title}\u0000${metadata.artist}\u0000${metadata.albumArtist}`
})
const displayedThumbnail = useTaskbarDisplayedThumbnail(mediaSession)
const { progressColor } = useTaskbarProgressColor(thumbnailDataUrl, trackIdentity)
const coverImage = computed(() => displayedThumbnail.value?.image ?? null)
const coverBackgroundActive = computed(() => backgroundMode.value !== 'theme')

const effectiveBackgroundTransparency = computed(() =>
  coverBackgroundActive.value ? 0 : backgroundTransparency.value,
)
const { foregroundColor, isSystemForegroundActive } = useTaskbarForegroundColor(
  effectiveBackgroundTransparency,
)
const activeForegroundColor = computed(() =>
  coverBackgroundActive.value ? 'oklch(1 0 0)' : foregroundColor.value,
)
const activeSecondaryForegroundColor = computed(() =>
  coverBackgroundActive.value
    ? 'oklch(0.96 0 0)'
    : isSystemForegroundActive.value
      ? `color-mix(in srgb, ${activeForegroundColor.value} 90%, transparent)`
      : 'var(--taskbar-secondary-foreground)',
)

/** 设置数组就是任务栏从左到右的最终 DOM 顺序。 */
const resolvedElementOrder = computed(() => normalizeTaskbarElementOrder(elementOrder.value))

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

/**
 * 在排列、控件可见性或窗口尺寸变化后刷新两个封面锚点。
 * 普通层在歌词模式下仍保持挂载，其文本变动会持续触发子树变更，因此必须节流，
 * 否则每次变更都会产生 3 次强制布局读取。
 */
const refreshCoverAnchors = useThrottleFn(
  () => {
    void nextTick(() => {
      contentBounds.update()
      normalCoverBounds.update()
      lyricsCoverBounds.update()
    })
  },
  100,
  true,
  true,
)

useMutationObserver(normalLayer, refreshCoverAnchors, { childList: true, subtree: true })
watch([resolvedElementOrder, () => coverAppearance.value.visibility], refreshCoverAnchors)

/** 封面模式使用不透明深色底层，白色前景不受系统模式和封面明度影响。 */
const backgroundStyle = computed<CSSProperties>(() => ({
  backgroundColor: !coverBackgroundActive.value
    ? `color-mix(in srgb, var(--taskbar-background) ${100 - backgroundTransparency.value}%, transparent)`
    : 'oklch(0.08 0 0)',
  '--taskbar-active-foreground': activeForegroundColor.value,
  '--taskbar-active-secondary-foreground': activeSecondaryForegroundColor.value,
  color: activeForegroundColor.value,
}))

/** 仅把解析后的主题色暴露给进度条，避免影响全局 primary 色。 */
const progressColorStyle = computed(() => ({
  '--taskbar-progress-color': progressColor.value,
}))

/**
 * 合并根节点样式为单个对象：其引用只在自身依赖变化时改变，播放中每帧重渲染会因此
 * 直接跳过整块 style 的归一化与逐属性 diff（数组写法每次渲染都会产生新引用）。
 */
const rootStyle = computed<CSSProperties>(() => ({
  ...backgroundStyle.value,
  ...progressColorStyle.value,
}))

/** 用合成器缩放已播放区域，避免播放进度变化触发布局。 */
const barProgressStyle = computed(() => ({
  width: '100%',
  transform: `scaleX(${progress.value / 100})`,
  transformOrigin: 'left center',
}))

const progressBarPositionClass = computed(() =>
  progressPosition.value === 'top' ? 'top-0' : 'bottom-0',
)

/** 右键开关当前媒体会话所属的播放器窗口：已打开时关闭，最小化或隐藏时打开。 */
async function togglePlayer() {
  try {
    await toggleCurrentMediaPlayer()
  } catch (error) {
    reportBackgroundFailure('开关当前播放器窗口失败', error)
  }
}

/** 用贴近任务栏背景的同色系渐变标示已播放区域，避免与歌词颜色混在一起。 */
const verticalProgressStyle = computed(() => ({
  width: '100%',
  transform: `scaleX(${progress.value / 100})`,
  transformOrigin: 'left center',
  background:
    'linear-gradient(to right, transparent 0%, color-mix(in srgb, var(--taskbar-progress-color) 40%, var(--taskbar-background)) 100%)',
}))

onMounted(refreshCoverAnchors)
</script>

<template>
  <main
    ref="taskbarRoot"
    class="text-taskbar-foreground relative flex size-full items-center gap-2 overflow-hidden px-2 py-1 shadow-sm select-none"
    :style="rootStyle"
    @contextmenu.prevent="togglePlayer"
  >
    <CoverBackgroundElement
      v-if="
        taskbarContentVisible &&
        coverBackgroundActive &&
        coverImage &&
        progressStyle !== 'vertical-gradient'
      "
      :image="coverImage"
    />
    <div ref="contentRoot" class="relative z-10 min-w-0 flex-1 self-stretch">
      <div
        ref="normalLayer"
        class="taskbar-mode-layer"
        :class="showLyrics ? 'pointer-events-none opacity-0' : 'opacity-100'"
        :aria-hidden="showLyrics"
        :inert="showLyrics || undefined"
      >
        <template v-for="element in resolvedElementOrder.normal" :key="element">
          <div
            v-if="element === 'cover' && normalCoverVisible"
            :ref="setNormalCoverAnchor"
            class="size-8 shrink-0"
            aria-hidden="true"
          />
          <TrackInfoElement
            v-else-if="element === 'track-info'"
            :session="mediaSession"
            :active="!showLyrics"
          />
          <PlaybackControlsElement
            v-else-if="element === 'controls'"
            :session="mediaSession"
            :pending="controlPending"
            :theme-color="progressColor"
            :compact="isCompact"
            @control="control"
          />
        </template>
      </div>

      <div
        class="taskbar-mode-layer pointer-events-none"
        :class="showLyrics ? 'opacity-100' : 'opacity-0'"
        :style="lyricsLayerStyle"
        :aria-hidden="!showLyrics"
      >
        <template v-for="element in resolvedElementOrder.lyrics" :key="element">
          <div
            v-if="element === 'cover' && lyricsCoverVisible"
            :ref="setLyricsCoverAnchor"
            class="size-8 shrink-0"
            aria-hidden="true"
          />
          <LyricsElement
            v-else-if="
              element === 'lyrics' &&
              taskbarContentVisible &&
              lyricsSettings.enabled &&
              lyrics.status === 'ready'
            "
            :key="lyrics.trackKey ?? 'no-track'"
            :lyrics="lyrics"
            :position-ms="lyricsPositionMs"
            :settings="lyricsSettings"
            :theme-color="progressColor"
          />
          <LyricsNoticeElement
            v-else-if="
              element === 'lyrics' &&
              taskbarContentVisible &&
              lyricsSettings.enabled &&
              hasNoticeOnly
            "
            :text="lyricsNoticeText"
            :settings="lyricsSettings"
            :theme-color="progressColor"
          />
        </template>
      </div>

      <div
        v-if="activeCoverVisible"
        class="taskbar-cover-motion pointer-events-none absolute top-0 left-0 z-20 size-8"
        :style="coverMotionStyle"
      >
        <CoverElement
          :session="mediaSession"
          :appearance="coverAppearance"
          :thumbnail-data-url="displayedThumbnail?.source ?? null"
        />
      </div>
    </div>

    <AudioSpectrumElement
      v-if="spectrumSettingsReady && taskbarContentVisible && spectrumSettings.visible"
      :settings="spectrumSettings"
      :theme-color="progressColor"
      :foreground-color="activeForegroundColor"
      :progress="progress"
      :overlaps-progress-gradient="progressStyle === 'vertical-gradient'"
    />

    <div
      v-if="timeline && progressVisible"
      class="pointer-events-none absolute inset-0"
      role="progressbar"
      :aria-label="t('media.progress')"
      aria-valuemin="0"
      aria-valuemax="100"
      :aria-valuenow="Math.round(progress)"
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
