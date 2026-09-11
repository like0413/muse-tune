<script setup lang="ts">
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'

defineProps<{
  title: string
  description: string
}>()
</script>

<template>
  <Card class="gap-4 py-5">
    <CardHeader class="px-5">
      <CardTitle class="flex items-center gap-2 text-sm">
        <slot name="icon" />
        {{ title }}
        <slot name="badge" />
      </CardTitle>
      <CardDescription>{{ description }}</CardDescription>
    </CardHeader>
    <CardContent class="px-5">
      <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-5 gap-y-2 text-sm">
        <slot />
      </dl>
    </CardContent>
  </Card>
</template>
