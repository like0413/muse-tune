<script setup lang="ts">
import AutostartSetting from './AutostartSetting.vue'
import ColorModeSetting from './ColorModeSetting.vue'
import LanguageSetting from './LanguageSetting.vue'
import ReducedMotionSetting from './ReducedMotionSetting.vue'
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <LanguageSetting />
    <ColorModeSetting />
    <ReducedMotionSetting />
    <AutostartSetting />
  </div>
</template>
