<script setup lang="ts">
import TaskbarBackgroundTransparencySetting from './TaskbarBackgroundTransparencySetting.vue'
import TaskbarDisplaySetting from './TaskbarDisplaySetting.vue'
import TaskbarOverlapPrioritySetting from './TaskbarOverlapPrioritySetting.vue'
import TaskbarPlacementSetting from './TaskbarPlacementSetting.vue'
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <TaskbarBackgroundTransparencySetting />
    <TaskbarDisplaySetting />
    <TaskbarPlacementSetting />
    <TaskbarOverlapPrioritySetting />
  </div>
</template>
