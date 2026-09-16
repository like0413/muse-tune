<script setup lang="ts">
import { Check, Database, FolderOpen, LoaderCircle, RefreshCw, Trash2 } from '@lucide/vue'

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
import { Progress } from '@/components/ui/progress'
import { formatBytes } from '@/features/data-management/format'
import type { CacheOverview } from '@/features/data-management/types'

const { t } = useI18n({ useScope: 'global' })

const props = defineProps<{
  overview: CacheOverview
  clearing: boolean
  opening: boolean
  cleared: boolean
  clearingCurrent: boolean
  refreshingCurrent: boolean
}>()

const emit = defineEmits<{
  clear: []
  open: []
  clearCurrent: []
  refreshCurrent: []
}>()

const usagePercentage = computed(() => {
  if (props.overview.capacityBytes <= 0) return 0
  return Math.min(100, (props.overview.usedBytes / props.overview.capacityBytes) * 100)
})
</script>

<template>
  <Item>
    <ItemContent>
      <ItemTitle>
        <Database class="size-4 text-indigo-500" />
        {{ t('settings.data.cache.title') }}
      </ItemTitle>
      <ItemDescription>{{ t('settings.data.cache.description') }}</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Button
        variant="outline"
        size="sm"
        :disabled="clearing || clearingCurrent || refreshingCurrent"
        @click="emit('clearCurrent')"
      >
        <LoaderCircle v-if="clearingCurrent" data-icon="inline-start" class="animate-spin" />
        <Trash2 v-else data-icon="inline-start" />
        {{ t('settings.data.cache.clearCurrent') }}
      </Button>
      <Button
        variant="outline"
        size="sm"
        :disabled="clearing || clearingCurrent || refreshingCurrent"
        @click="emit('refreshCurrent')"
      >
        <LoaderCircle v-if="refreshingCurrent" data-icon="inline-start" class="animate-spin" />
        <RefreshCw v-else data-icon="inline-start" />
        {{ t('settings.data.cache.refreshCurrent') }}
      </Button>
      <AlertDialog>
        <AlertDialogTrigger as-child>
          <Button
            variant="outline"
            size="sm"
            :disabled="clearing || clearingCurrent || refreshingCurrent"
          >
            <LoaderCircle v-if="clearing" data-icon="inline-start" class="animate-spin" />
            <Check v-else-if="cleared" data-icon="inline-start" />
            <Trash2 v-else data-icon="inline-start" />
            {{ cleared ? t('common.cleared') : t('settings.data.cache.clearAll') }}
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent class="w-100">
          <AlertDialogHeader>
            <AlertDialogTitle>{{ t('settings.data.cache.clearConfirmTitle') }}</AlertDialogTitle>
            <AlertDialogDescription>
              {{ t('settings.data.cache.clearConfirmDescription') }}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{{ t('common.cancel') }}</AlertDialogCancel>
            <AlertDialogAction
              class="bg-destructive hover:bg-destructive/90 text-white"
              @click="emit('clear')"
            >
              {{ t('common.clear') }}
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
      <div class="flex w-full flex-col gap-2">
        <div class="flex items-center justify-between gap-4">
          <div class="flex items-center gap-2">
            <Badge variant="secondary">{{
              t('settings.data.cache.entries', { count: overview.entryCount })
            }}</Badge>
            <span class="text-muted-foreground text-xs">{{
              t('settings.data.cache.fileCount')
            }}</span>
          </div>
          <div class="flex items-baseline gap-2 tabular-nums">
            <span class="text-sm font-semibold">{{ formatBytes(overview.usedBytes) }}</span>
            <span class="text-muted-foreground text-xs">
              {{
                t('settings.data.usedCapacity', { capacity: formatBytes(overview.capacityBytes) })
              }}
            </span>
          </div>
        </div>
        <Progress :model-value="usagePercentage" :aria-label="t('settings.data.cache.capacity')" />
      </div>
    </ItemFooter>
  </Item>
</template>
