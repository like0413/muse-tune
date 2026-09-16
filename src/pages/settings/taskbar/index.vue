<script setup lang="ts">
import AudioSpectrumSetting from './AudioSpectrumSetting.vue'
import BackgroundStyleSetting from './BackgroundStyleSetting.vue'
import BackgroundTransparencySetting from './BackgroundTransparencySetting.vue'
import BarVisibilitySetting from './BarVisibilitySetting.vue'
import BarWidthSetting from './BarWidthSetting.vue'
import ControlsSetting from './ControlsSetting.vue'
import CoverSetting from './CoverSetting.vue'
import DisplaySetting from './DisplaySetting.vue'
import ElementOrderSetting from './ElementOrderSetting.vue'
import LyricsDisplaySetting from './lyrics/LyricsDisplaySetting.vue'
import MediaSessionSetting from './media-session/MediaSessionSetting.vue'
import OverlapPrioritySetting from './OverlapPrioritySetting.vue'
import PlacementSetting from './PlacementSetting.vue'
import ProgressStyleSetting from './ProgressStyleSetting.vue'
import ThemeColorSetting from './ThemeColorSetting.vue'
import TrackInfoSetting from './track-info/TrackInfoSetting.vue'

const { t } = useI18n({ useScope: 'global' })
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <div class="pl-2 text-sm font-bold">{{ t('settings.taskbar.groups.placement') }}</div>
    <PlacementSetting />
    <OverlapPrioritySetting />
    <BarVisibilitySetting />
    <DisplaySetting />

    <div class="pl-2 text-sm font-bold">{{ t('settings.taskbar.groups.appearance') }}</div>
    <BarWidthSetting />
    <ElementOrderSetting />
    <ThemeColorSetting />
    <BackgroundStyleSetting />
    <BackgroundTransparencySetting />

    <div class="pl-2 text-sm font-bold">{{ t('settings.taskbar.groups.content') }}</div>
    <CoverSetting />
    <TrackInfoSetting />
    <LyricsDisplaySetting />
    <ControlsSetting />
    <ProgressStyleSetting />
    <AudioSpectrumSetting />

    <div class="pl-2 text-sm font-bold">{{ t('settings.taskbar.groups.advanced') }}</div>
    <MediaSessionSetting />
  </div>
</template>
