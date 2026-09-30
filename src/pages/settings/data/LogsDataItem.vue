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
import { Item, ItemActions, ItemContent, ItemFooter, ItemTitle } from '@/components/ui/item'
import type { LogsOverview } from '@/features/data-management/types'
import { formatBytes } from '@/features/i18n/format'

const { t } = useI18n({ useScope: 'global' })

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
        {{ t('settings.data.logs.title') }}
      </ItemTitle>
    </ItemContent>
    <ItemActions>
      <AlertDialog>
        <AlertDialogTrigger as-child>
          <Button variant="outline" size="sm" :disabled="clearing || !hasHistory">
            <LoaderCircle v-if="clearing" data-icon="inline-start" class="animate-spin" />
            <Trash2 v-else data-icon="inline-start" />
            {{ cleared ? t('common.cleared') : t('settings.data.logs.clearHistory') }}
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent class="w-100">
          <AlertDialogHeader>
            <AlertDialogTitle>{{ t('settings.data.logs.clearConfirmTitle') }}</AlertDialogTitle>
            <AlertDialogDescription>
              {{ t('settings.data.logs.clearConfirmDescription') }}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{{ t('common.cancel') }}</AlertDialogCancel>
            <AlertDialogAction
              class="bg-destructive hover:bg-destructive/90 text-white"
              @click="emit('clear')"
            >
              {{ t('settings.data.logs.clearHistory') }}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <Button variant="outline" size="sm" :disabled="opening" @click="emit('open')">
        <LoaderCircle v-if="opening" data-icon="inline-start" class="animate-spin" />
        <FolderOpen v-else data-icon="inline-start" />
        {{ t('common.openDirectory') }}
      </Button>
    </ItemActions>
    <ItemFooter>
      <div class="flex w-full items-center justify-between gap-4">
        <div class="flex items-center gap-2">
          <Badge variant="secondary">{{
            t('settings.data.files', { count: overview.fileCount })
          }}</Badge>
          <span class="text-muted-foreground text-xs">{{
            t('settings.data.logs.fileDescription')
          }}</span>
        </div>
        <div class="flex items-baseline gap-2 tabular-nums">
          <span class="text-sm font-semibold">{{ formatBytes(overview.totalBytes) }}</span>
          <span class="text-muted-foreground text-xs">
            {{
              t('settings.data.usedApproximateCapacity', {
                capacity: formatBytes(overview.capacityBytes),
              })
            }}
          </span>
        </div>
      </div>
    </ItemFooter>
  </Item>
</template>
