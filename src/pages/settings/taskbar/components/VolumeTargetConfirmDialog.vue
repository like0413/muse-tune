<script setup lang="ts">
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

defineProps<{ disabled: boolean }>()
const emit = defineEmits<{ keep: []; restore: [] }>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n({ useScope: 'global' })
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent :show-close-button="false">
      <DialogHeader>
        <DialogTitle>{{ t('settings.taskbar.controls.volumeRestoreTitle') }}</DialogTitle>
        <DialogDescription>
          {{ t('settings.taskbar.controls.volumeRestoreDescription') }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button type="button" variant="outline" :disabled="disabled" @click="emit('keep')">
          {{ t('settings.taskbar.controls.keepAndSwitch') }}
        </Button>
        <Button type="button" :disabled="disabled" @click="emit('restore')">
          {{ t('settings.taskbar.controls.restoreAndSwitch') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
