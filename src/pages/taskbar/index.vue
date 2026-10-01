<script setup lang="ts">
import {
  useElementBounding,
  useElementHover,
  useMutationObserver,
  useThrottleFn,
} from '@vueuse/core'
import type { CSSProperties, VNodeRef } from 'vue'

import { reportBackgroundFailure } from '@/features/feedback/errors'
import { getApplicationLocaleTag } from '@/features/i18n/locales'
import { useConvertedChineseTexts } from '@/features/lyrics/useConvertedChineseTexts'
import { useLyrics } from '@/features/lyrics/useLyrics'
import { useTaskbarLyricsSettings } from '@/features/lyrics/useTaskbarLyricsSettings'
import { toggleCurrentMediaPlayer } from '@/features/media/client'
import { useMediaProgress } from '@/features/media/useMediaProgress'
import { useMediaSession } from '@/features/media/useMediaSession'
import { useMediaSessionSelectionPolicy } from '@/features/media/useMediaSessionSelectionPolicy'
import { useTaskbarInteractionSounds } from '@/features/media/useTaskbarInteractionSounds'
import { useVolumeControl } from '@/features/media/useVolumeControl'
import { useTaskbarAudioSpectrumSettings } from '@/features/settings/audio-spectrum'
import { TASKBAR_WIDTH_PRESETS } from '@/features/settings/bar-width'
import { isTaskbarCoverVisibleInMode } from '@/features/settings/cover'
import { normalizeTaskbarElementOrder } from '@/features/settings/element-order'
import { resolveLyricsChineseVariant } from '@/features/settings/lyrics'
import { provideTaskbarPlaybackClock } from '@/features/taskbar/playback-clock'
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
import PlaybackProgressElement from './components/PlaybackProgressElement.vue'
import TrackInfoElement from './components/TrackInfoElement.vue'
import VolumeSliderOverlay from './components/VolumeSliderOverlay.vue'

const { locale, t } = useI18n({ useScope: 'global' })
const { session: mediaSession, timeline, controlPending, control } = useMediaSession()
const playbackStatus = computed(() => mediaSession.value?.playback.status ?? 'unknown')
const { settings: lyricsSettings } = useTaskbarLyricsSettings()
/** 歌曲信息和歌词使用同一实际字形目标，跟随界面时由当前语言决定。 */
const effectiveChineseVariant = computed(() =>
  resolveLyricsChineseVariant(
    lyricsSettings.value.chineseVariant,
    getApplicationLocaleTag(locale.value),
  ),
)
const rawTrackTexts = computed(() => {
  const metadata = mediaSession.value?.metadata
  return [
    metadata?.title || t('media.nothingPlaying'),
    metadata?.artist || metadata?.albumArtist || metadata?.subtitle || '—',
  ]
})
const { convertedTexts: trackTexts } = useConvertedChineseTexts(
  rawTrackTexts,
  effectiveChineseVariant,
)
const trackTitle = computed(() => trackTexts.value[0] ?? rawTrackTexts.value[0] ?? '')
const trackArtist = computed(() => trackTexts.value[1] ?? rawTrackTexts.value[1] ?? '—')
const { lyrics } = useLyrics(computed(() => lyricsSettings.value.enabled))
const { settings: spectrumSettings, ready: spectrumSettingsReady } =
  useTaskbarAudioSpectrumSettings()
const { appearance: coverAppearance } = useTaskbarCoverAppearance()
const taskbarRoot = useTemplateRef<HTMLElement>('taskbarRoot')
const contentRoot = useTemplateRef<HTMLElement>('contentRoot')
const normalLayer = useTemplateRef<HTMLElement>('normalLayer')
const normalCoverAnchor = shallowRef<HTMLElement | null>(null)
const lyricsCoverAnchor = shallowRef<HTMLElement | null>(null)
const volumeOverlayVisible = shallowRef(false)
const volumeOverlayReturnsToLyrics = shallowRef(false)
const volumeOverlayRestoringLyrics = shallowRef(false)
const { target: volumeTarget, volume, setLevel, adjustLevel, toggleMuted } = useVolumeControl()
const { playPlayerToggleSound, playVolumeStepSound } = useTaskbarInteractionSounds()
const volumePercentage = computed(() => Math.round((volume.value?.level ?? 0) * 100))

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
// 纯音乐没有可展示内容，直接回退普通模式（见 `hasLyricsContent`）；
// 只剩“没有歌词”还留在歌词模式里给提示。
const hasNoticeOnly = computed(() => lyrics.value.status === 'no_lyrics')
/** 结论型状态的提示文案；与 `hasNoticeOnly` 覆盖的状态一一对应。 */
const lyricsNoticeText = computed(() => t('taskbar.lyrics.noLyrics'))
const hasLyricsContent = computed(() => hasTimedLyrics.value || hasNoticeOnly.value)
/** 歌词是否需要持续推进时间轴；与刷新是否平滑无关。 */
const needsLyricsTimeline = computed(
  () =>
    lyricsSettings.value.enabled &&
    hasTimedLyrics.value &&
    playbackStatus.value === 'playing' &&
    !isTaskbarHovered.value,
)
// 逐字高亮属于歌词内容进度，不是装饰性动画；减少动态效果不能降低它的更新时间精度。
const needsSmoothProgress = needsLyricsTimeline
const needsProgress = computed(
  () =>
    taskbarContentVisible.value &&
    !volumeOverlayVisible.value &&
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
provideTaskbarPlaybackClock({ progress, lyricsPositionMs })
// 所有解析入口都要求有效播放器时间线；纯音乐结论本身不伪装成歌词行。
const hasReliableLyricsTimeline = computed(() => timeline.value !== null)
/** 不含 hover 的歌词模式资格，供音量层记录退出后应恢复的内容。 */
const lyricsModeAvailable = computed(
  () =>
    lyricsSettings.value.enabled &&
    playbackStatus.value === 'playing' &&
    hasReliableLyricsTimeline.value &&
    hasLyricsContent.value,
)
const showLyrics = computed(
  () =>
    lyricsModeAvailable.value &&
    (!isTaskbarHovered.value ||
      (volumeOverlayVisible.value && volumeOverlayReturnsToLyrics.value) ||
      volumeOverlayRestoringLyrics.value),
)
/** 音量层遮挡期间直接准备目标内容，避免关闭时播放普通/歌词层的交叉淡入。 */
const suppressModeTransition = computed(
  () => volumeOverlayVisible.value || volumeOverlayRestoringLyrics.value,
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
const { foregroundColor } = useTaskbarForegroundColor(effectiveBackgroundTransparency)
const activeForegroundColor = computed(() =>
  coverBackgroundActive.value ? 'oklch(1 0 0)' : foregroundColor.value,
)
/** 歌手、默认歌词未播放文字和第二行统一使用 96% 前景强度，并保留浅色背景对比度。 */
const activeSecondaryForegroundColor = computed(() =>
  coverBackgroundActive.value
    ? 'oklch(0.96 0 0)'
    : `color-mix(in srgb, ${activeForegroundColor.value} 96%, var(--taskbar-background))`,
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

/** 显示最新音量；鼠标仍位于 bar 内时保持展示。 */
function showVolumeOverlay() {
  volumeOverlayReturnsToLyrics.value = lyricsModeAvailable.value
  volumeOverlayRestoringLyrics.value = false
  volumeOverlayVisible.value = true
}

/** 音量对象失效时同步收起弹层。 */
function hideVolumeOverlay() {
  volumeOverlayVisible.value = false
  volumeOverlayReturnsToLyrics.value = false
  volumeOverlayRestoringLyrics.value = false
}

/**
 * 移出时先固定应恢复的歌词层再卸载弹层；hover 状态完成同步后再释放固定状态。
 */
function handleTaskbarPointerLeave() {
  if (!volumeOverlayVisible.value) return
  volumeOverlayRestoringLyrics.value = volumeOverlayReturnsToLyrics.value
  volumeOverlayVisible.value = false
  volumeOverlayReturnsToLyrics.value = false
}

/** 鼠标位于整条任务栏播放器上时，用滚轮按 2% 调整当前选择的音量对象。 */
function handleVolumeWheel(event: WheelEvent) {
  if (!volume.value || event.deltaY === 0) return
  event.preventDefault()
  const previousPercentage = volumePercentage.value
  adjustLevel(event.deltaY < 0 ? 1 : -1)
  if (volumePercentage.value !== previousPercentage) playVolumeStepSound()
  showVolumeOverlay()
}

/** 让弹层内的滑杆与滚轮共用同一音量入口并保持展示。 */
function handleVolumeLevel(level: number) {
  if (Math.round(level * 100) === volumePercentage.value) return
  setLevel(level)
  playVolumeStepSound()
  showVolumeOverlay()
}

function handleVolumeAdjustment(direction: 1 | -1) {
  const previousPercentage = volumePercentage.value
  adjustLevel(direction)
  if (volumePercentage.value !== previousPercentage) playVolumeStepSound()
  showVolumeOverlay()
}

/** 保留弹层内的静音入口，并在点击后继续展示更新后的状态。 */
function handleVolumeMutedToggle() {
  toggleMuted()
  showVolumeOverlay()
}

watch(volume, (value) => {
  if (!value) hideVolumeOverlay()
})
watch(isTaskbarHovered, (hovered) => {
  if (!hovered) volumeOverlayRestoringLyrics.value = false
})

/** 右键开关当前媒体会话所属的播放器窗口：已打开时关闭，最小化或隐藏时打开。 */
async function togglePlayer() {
  try {
    await toggleCurrentMediaPlayer()
    playPlayerToggleSound()
  } catch (error) {
    reportBackgroundFailure('开关当前播放器窗口失败', error)
  }
}

onMounted(refreshCoverAnchors)
</script>

<template>
  <main
    ref="taskbarRoot"
    class="text-taskbar-foreground relative flex size-full items-center gap-2 overflow-hidden px-2 py-1 shadow-sm select-none"
    :style="rootStyle"
    @contextmenu.prevent="togglePlayer"
    @wheel="handleVolumeWheel"
    @pointerleave="handleTaskbarPointerLeave"
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
    <div
      ref="contentRoot"
      class="relative z-10 min-w-0 flex-1 self-stretch"
      :class="volumeOverlayVisible ? 'pointer-events-none opacity-0' : 'opacity-100'"
    >
      <div
        ref="normalLayer"
        class="taskbar-mode-layer"
        :class="[
          showLyrics ? 'pointer-events-none opacity-0' : 'opacity-100',
          suppressModeTransition && 'transition-none',
        ]"
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
            :title="trackTitle"
            :artist="trackArtist"
            :active="!showLyrics && taskbarContentVisible && !volumeOverlayVisible"
          />
          <PlaybackControlsElement
            v-else-if="element === 'controls'"
            :session="mediaSession"
            :pending="controlPending"
            :compact="isCompact"
            @control="control"
          />
        </template>
      </div>

      <div
        class="taskbar-mode-layer pointer-events-none"
        :class="[
          showLyrics ? 'opacity-100' : 'opacity-0',
          suppressModeTransition && 'transition-none',
        ]"
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
          :active="taskbarContentVisible && !volumeOverlayVisible"
          :session="mediaSession"
          :appearance="coverAppearance"
          :thumbnail-data-url="displayedThumbnail?.source ?? null"
          :progress-color="progressColor"
          :show-progress-ring="
            Boolean(timeline) && progressVisible && progressStyle === 'cover-ring'
          "
        />
      </div>
    </div>

    <AudioSpectrumElement
      v-if="
        !volumeOverlayVisible &&
        spectrumSettingsReady &&
        taskbarContentVisible &&
        spectrumSettings.visible
      "
      :settings="spectrumSettings"
      :theme-color="progressColor"
      :foreground-color="activeForegroundColor"
      :overlaps-progress-gradient="progressStyle === 'vertical-gradient'"
    />

    <PlaybackProgressElement
      v-if="!volumeOverlayVisible && timeline && progressVisible"
      :mode="progressStyle"
      :position="progressPosition"
    />

    <VolumeSliderOverlay
      v-if="volumeOverlayVisible"
      :target="volumeTarget"
      :percentage="volumePercentage"
      :muted="volume?.muted ?? false"
      :theme-color="progressColor"
      :disabled="!volume"
      @set-level="handleVolumeLevel"
      @adjust-level="handleVolumeAdjustment"
      @toggle-muted="handleVolumeMutedToggle"
    />
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
