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
    <ItemMedia class="icon-tone-sky-500">
      <ScrollText />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>日志</ItemTitle>
      <ItemDescription>运行日志与错误记录 · {{ detail }}</ItemDescription>
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
  </Item>
</template>
