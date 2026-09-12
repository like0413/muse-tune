<script setup lang="ts">
import { FolderOpen, LoaderCircle, ScrollText } from '@lucide/vue'

import { Button } from '@/components/ui/button'
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'

defineProps<{
  detail: string
  opening: boolean
}>()

const emit = defineEmits<{ open: [] }>()
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-sky-500">
      <ScrollText />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>日志</ItemTitle>
      <ItemDescription>运行日志与错误记录 · {{ detail }}</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Button variant="outline" size="sm" :disabled="opening" @click="emit('open')">
        <LoaderCircle v-if="opening" data-icon="inline-start" class="animate-spin" />
        <FolderOpen v-else data-icon="inline-start" />
        打开目录
      </Button>
    </ItemActions>
  </Item>
</template>
