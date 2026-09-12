<script setup lang="ts">
import { FileCog, FolderOpen, LoaderCircle, RotateCcw } from '@lucide/vue'

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
  resetting: boolean
}>()

const emit = defineEmits<{
  open: []
  reset: []
}>()
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-amber-500">
      <FileCog />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>应用配置</ItemTitle>
      <ItemDescription>应用设置和用户偏好 · {{ detail }}</ItemDescription>
    </ItemContent>
    <ItemActions>
      <AlertDialog>
        <AlertDialogTrigger as-child>
          <Button variant="outline" size="sm" :disabled="resetting">
            <LoaderCircle v-if="resetting" data-icon="inline-start" class="animate-spin" />
            <RotateCcw v-else data-icon="inline-start" />
            重置配置
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent class="w-100">
          <AlertDialogHeader>
            <AlertDialogTitle>重置全部配置？</AlertDialogTitle>
            <AlertDialogDescription>
              所有设置将恢复默认值，Muse Tune 随后会自动重启。此操作无法撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              class="bg-destructive hover:bg-destructive/90 text-white"
              @click="emit('reset')"
            >
              重置并重启
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
