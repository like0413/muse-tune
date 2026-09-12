<script setup lang="ts">
import { Check, Database, FolderOpen, LoaderCircle, Trash2 } from '@lucide/vue'
import { computed } from 'vue'

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemFooter,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Progress } from '@/components/ui/progress'
import { formatBytes } from '@/features/data-management/format'
import type { CacheOverview } from '@/features/data-management/types'

const props = defineProps<{
  overview: CacheOverview
  clearing: boolean
  opening: boolean
  cleared: boolean
}>()

const emit = defineEmits<{
  clear: []
  open: []
}>()

const usagePercentage = computed(() => {
  if (props.overview.capacityBytes <= 0) return 0
  return Math.min(100, (props.overview.usedBytes / props.overview.capacityBytes) * 100)
})
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-indigo-500">
      <Database />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>歌词缓存</ItemTitle>
      <ItemDescription>MuseTune 维护的歌词缓存数据</ItemDescription>
    </ItemContent>
    <ItemActions>
      <AlertDialog>
        <AlertDialogTrigger as-child>
          <Button variant="outline" size="sm" :disabled="clearing">
            <LoaderCircle v-if="clearing" data-icon="inline-start" class="animate-spin" />
            <Check v-else-if="cleared" data-icon="inline-start" />
            <Trash2 v-else data-icon="inline-start" />
            {{ cleared ? '已清理' : '清理缓存' }}
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent class="w-100">
          <AlertDialogHeader>
            <AlertDialogTitle>清理全部歌词缓存？</AlertDialogTitle>
            <AlertDialogDescription>
              已缓存的歌词将被删除，之后播放歌曲时会按需重新获取。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              class="bg-destructive hover:bg-destructive/90 text-white"
              @click="emit('clear')"
            >
              清理
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <Button variant="outline" size="sm" :disabled="opening" @click="emit('open')">
        <LoaderCircle v-if="opening" data-icon="inline-start" class="animate-spin" />
        <FolderOpen v-else data-icon="inline-start" />
        打开目录
      </Button>
    </ItemActions>
    <ItemFooter class="pl-10">
      <div class="flex w-full flex-col gap-2">
        <div class="flex items-center justify-between gap-4 text-xs">
          <span class="font-medium">{{ formatBytes(overview.usedBytes) }} 已使用</span>
          <span class="text-muted-foreground font-medium">
            {{ formatBytes(overview.capacityBytes) }} · {{ overview.entryCount }} 项
          </span>
        </div>
        <Progress :model-value="usagePercentage" aria-label="歌词缓存容量" />
      </div>
    </ItemFooter>
  </Item>
</template>
