<script setup lang="ts">
import { FolderOpen, LoaderCircle, ScrollText, Trash2 } from '@lucide/vue'

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
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemFooter,
  ItemTitle,
} from '@/components/ui/item'
import { formatBytes } from '@/features/data-management/format'
import type { LogsOverview } from '@/features/data-management/types'

defineProps<{
  overview: LogsOverview
  opening: boolean
  clearing: boolean
  cleared: boolean
  hasHistory: boolean
}>()

const emit = defineEmits<{
  clear: []
  open: []
}>()
</script>

<template>
  <Item>
    <ItemContent>
      <ItemTitle>
        <ScrollText class="size-4 text-sky-500" />
        日志
      </ItemTitle>
      <ItemDescription>运行日志与错误记录</ItemDescription>
    </ItemContent>
    <ItemActions>
      <AlertDialog>
        <AlertDialogTrigger as-child>
          <Button variant="outline" size="sm" :disabled="clearing || !hasHistory">
            <LoaderCircle v-if="clearing" data-icon="inline-start" class="animate-spin" />
            <Trash2 v-else data-icon="inline-start" />
            {{ cleared ? '已清理' : '清理历史日志' }}
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent class="w-100">
          <AlertDialogHeader>
            <AlertDialogTitle>清理历史日志？</AlertDialogTitle>
            <AlertDialogDescription>
              已轮转的历史日志将被删除；当前日志会保留并继续写入，无需重启应用。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              class="bg-destructive hover:bg-destructive/90 text-white"
              @click="emit('clear')"
            >
              清理历史日志
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
    <ItemFooter>
      <div class="flex w-full items-center justify-between gap-4">
        <div class="flex items-center gap-2">
          <Badge variant="secondary">{{ overview.fileCount }} 个文件</Badge>
          <span class="text-muted-foreground text-xs">含当前日志与轮转历史</span>
        </div>
        <div class="flex items-baseline gap-2 tabular-nums">
          <span class="text-sm font-semibold">{{ formatBytes(overview.totalBytes) }}</span>
          <span class="text-muted-foreground text-xs">
            已用 / 约 {{ formatBytes(overview.capacityBytes) }} 上限
          </span>
        </div>
      </div>
    </ItemFooter>
  </Item>
</template>
