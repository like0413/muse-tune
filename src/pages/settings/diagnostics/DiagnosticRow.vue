<script setup lang="ts">
const { t } = useI18n({ useScope: 'global' })

defineProps<{
  label: string
  value?: string | number
  breakAll?: boolean
}>()
</script>

<template>
  <dt class="text-muted-foreground">{{ label }}</dt>
  <dd :class="{ 'min-w-0 break-all': breakAll }">
    <slot>{{ value ?? t('common.unavailable') }}</slot>
  </dd>
</template>
