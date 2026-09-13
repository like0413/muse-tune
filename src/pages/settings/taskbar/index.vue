<script setup lang="ts">
import AudioSpectrumSetting from './AudioSpectrumSetting.vue'
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
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <div class="pl-2 text-sm font-bold">位置与行为</div>
    <DisplaySetting />
    <PlacementSetting />
    <BarVisibilitySetting />
    <OverlapPrioritySetting />

    <div class="pl-2 text-sm font-bold">布局与外观</div>
    <BarWidthSetting />
    <ElementOrderSetting />
    <ThemeColorSetting />
    <BackgroundTransparencySetting />

    <div class="pl-2 text-sm font-bold">内容组件</div>
    <CoverSetting />
    <TrackInfoSetting />
    <LyricsDisplaySetting />
    <ControlsSetting />
    <ProgressStyleSetting />
    <AudioSpectrumSetting />

    <div class="pl-2 text-sm font-bold">高级设置</div>
    <MediaSessionSetting />
  </div>
</template>
