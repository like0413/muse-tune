<script setup lang="ts">
import BackgroundTransparencySetting from './BackgroundTransparencySetting.vue'
import BarVisibilitySetting from './BarVisibilitySetting.vue'
import BarWidthSetting from './BarWidthSetting.vue'
import ControlsSetting from './ControlsSetting.vue'
import CoverSetting from './CoverSetting.vue'
import DisplaySetting from './DisplaySetting.vue'
import ElementOrderSetting from './ElementOrderSetting.vue'
import MediaSessionSetting from './media-session/MediaSessionSetting.vue'
import OverlapPrioritySetting from './OverlapPrioritySetting.vue'
import PlacementSetting from './PlacementSetting.vue'
import ProgressStyleSetting from './ProgressStyleSetting.vue'
import ThemeColorSetting from './ThemeColorSetting.vue'
import TrackInfoSetting from './track-info/TrackInfoSetting.vue'
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <BackgroundTransparencySetting />
    <BarVisibilitySetting />
    <BarWidthSetting />
    <ProgressStyleSetting />
    <ThemeColorSetting />
    <ElementOrderSetting />
    <MediaSessionSetting />
    <TrackInfoSetting />
    <CoverSetting />
    <ControlsSetting />
    <DisplaySetting />
    <PlacementSetting />
    <OverlapPrioritySetting />
  </div>
</template>
