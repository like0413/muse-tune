<script setup lang="ts">
import BackgroundTransparencySetting from './BackgroundTransparencySetting.vue'
import BarWidthSetting from './BarWidthSetting.vue'
import DisplaySetting from './DisplaySetting.vue'
import OverlapPrioritySetting from './OverlapPrioritySetting.vue'
import PlacementSetting from './PlacementSetting.vue'
import ProgressStyleSetting from './ProgressStyleSetting.vue'
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <BackgroundTransparencySetting />
    <BarWidthSetting />
    <ProgressStyleSetting />
    <DisplaySetting />
    <PlacementSetting />
    <OverlapPrioritySetting />
  </div>
</template>
