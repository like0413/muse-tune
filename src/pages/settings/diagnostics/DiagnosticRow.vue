<script setup lang="ts">
defineProps<{
  label: string
  value?: string | number
  breakAll?: boolean
}>()
</script>

<template>
  <dt class="text-muted-foreground">{{ label }}</dt>
  <dd :class="{ 'min-w-0 break-all': breakAll }">
    <slot>{{ value ?? '暂无' }}</slot>
  </dd>
</template>
