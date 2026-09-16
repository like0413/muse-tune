<script setup lang="ts">
import { useApplicationLocaleSync } from '@/features/i18n/useApplicationLocaleSync'
import { useReducedMotionPreference } from '@/features/motion/useReducedMotionPreference'

useApplicationLocaleSync()
useReducedMotionPreference()
</script>

<template>
  <RouterView />
</template>
