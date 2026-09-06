<script setup lang="ts">
import BackgroundTransparencySetting from './BackgroundTransparencySetting.vue'
import BarWidthSetting from './BarWidthSetting.vue'
import ControlsSetting from './ControlsSetting.vue'
import CoverSetting from './CoverSetting.vue'
import DisplaySetting from './DisplaySetting.vue'
import ElementOrderSetting from './ElementOrderSetting.vue'
import OverlapPrioritySetting from './OverlapPrioritySetting.vue'
import PlacementSetting from './PlacementSetting.vue'
import ProgressStyleSetting from './ProgressStyleSetting.vue'
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <BackgroundTransparencySetting />
    <BarWidthSetting />
    <ProgressStyleSetting />
    <ElementOrderSetting />
    <CoverSetting />
    <ControlsSetting />
    <DisplaySetting />
    <PlacementSetting />
    <OverlapPrioritySetting />
  </div>
</template>
