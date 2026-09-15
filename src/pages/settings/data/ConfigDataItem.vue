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
import type { ConfigOverview } from '@/features/data-management/types'

const { t } = useI18n({ useScope: 'global' })

defineProps<{
  overview: ConfigOverview
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
    <ItemContent>
      <ItemTitle>
        <FileCog class="size-4 text-amber-500" />
        {{ t('settings.data.config.title') }}
      </ItemTitle>
      <ItemDescription>{{ t('settings.data.config.description') }}</ItemDescription>
    </ItemContent>
    <ItemActions>
      <AlertDialog>
        <AlertDialogTrigger as-child>
          <Button variant="outline" size="sm" :disabled="resetting">
            <LoaderCircle v-if="resetting" data-icon="inline-start" class="animate-spin" />
            <RotateCcw v-else data-icon="inline-start" />
            {{ t('settings.data.config.reset') }}
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent class="w-100">
          <AlertDialogHeader>
            <AlertDialogTitle>{{ t('settings.data.config.resetConfirmTitle') }}</AlertDialogTitle>
            <AlertDialogDescription>
              {{ t('settings.data.config.resetConfirmDescription') }}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{{ t('common.cancel') }}</AlertDialogCancel>
            <AlertDialogAction
              class="bg-destructive hover:bg-destructive/90 text-white"
              @click="emit('reset')"
            >
              {{ t('settings.data.config.resetAndRestart') }}
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
        <Badge variant="secondary">
          {{ overview.settingsFileExists ? t('settings.data.oneFile') : t('settings.data.noFile') }}
        </Badge>
        <div class="flex items-baseline gap-2 tabular-nums">
          <span class="text-sm font-semibold">
            {{ overview.settingsFileExists ? formatBytes(overview.settingsFileBytes) : '—' }}
          </span>
          <span class="text-muted-foreground text-xs">{{
            t('settings.data.config.fileSize')
          }}</span>
        </div>
      </div>
    </ItemFooter>
  </Item>
</template>
