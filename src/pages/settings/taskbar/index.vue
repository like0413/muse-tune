<script setup lang="ts">
import BackgroundTransparencySetting from './BackgroundTransparencySetting.vue'
import DisplaySetting from './DisplaySetting.vue'
import OverlapPrioritySetting from './OverlapPrioritySetting.vue'
import PlacementSetting from './PlacementSetting.vue'
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <BackgroundTransparencySetting />
    <DisplaySetting />
    <PlacementSetting />
    <OverlapPrioritySetting />
  </div>
</template>
