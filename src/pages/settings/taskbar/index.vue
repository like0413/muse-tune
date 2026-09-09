<script setup lang="ts">
import AudioSpectrumSetting from './AudioSpectrumSetting.vue'
import BackgroundTransparencySetting from './BackgroundTransparencySetting.vue'
import BarVisibilitySetting from './BarVisibilitySetting.vue'
import BarWidthSetting from './BarWidthSetting.vue'
import ControlsSetting from './ControlsSetting.vue'
import CoverSetting from './CoverSetting.vue'
import DisplaySetting from './DisplaySetting.vue'
import ElementOrderSetting from './ElementOrderSetting.vue'
import LyricsCacheSetting from './lyrics/LyricsCacheSetting.vue'
import LyricsDisplaySetting from './lyrics/LyricsDisplaySetting.vue'
import MediaSessionSetting from './media-session/MediaSessionSetting.vue'
import OverlapPrioritySetting from './OverlapPrioritySetting.vue'
import PlacementSetting from './PlacementSetting.vue'
import ProgressStyleSetting from './ProgressStyleSetting.vue'
import ThemeColorSetting from './ThemeColorSetting.vue'
import TrackInfoSetting from './track-info/TrackInfoSetting.vue'
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <div class="pl-2 text-sm font-bold">基础设置</div>
    <PlacementSetting />
    <OverlapPrioritySetting />
    <MediaSessionSetting />
    <BackgroundTransparencySetting />
    <BarWidthSetting />
    <ThemeColorSetting />
    <ElementOrderSetting />
    <div class="pl-2 text-sm font-bold">组件调整</div>
    <ProgressStyleSetting />
    <CoverSetting />
    <TrackInfoSetting />
    <ControlsSetting />
    <AudioSpectrumSetting />
    <LyricsDisplaySetting />
    <LyricsCacheSetting />
    <div class="pl-2 text-sm font-bold">其他</div>
    <BarVisibilitySetting />
    <DisplaySetting />
  </div>
</template>
