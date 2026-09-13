<script setup lang="ts">
import { RouterView } from 'vue-router'

import { useReducedMotionPreference } from '@/features/motion/useReducedMotionPreference'

useReducedMotionPreference()
</script>

<template>
  <RouterView />
</template>
