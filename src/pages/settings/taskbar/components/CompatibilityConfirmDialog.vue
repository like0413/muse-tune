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

defineProps<{ description: string }>()
const emit = defineEmits<{ confirm: [] }>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n({ useScope: 'global' })

/** 提交确认事件；父组件只在此事件中应用待确认设置。 */
function confirmChange() {
  emit('confirm')
  open.value = false
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{{ t('settings.taskbar.backgroundStyle.compatibilityTitle') }}</DialogTitle>
        <DialogDescription>{{ description }}</DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button type="button" variant="outline" @click="open = false">
          {{ t('common.cancel') }}
        </Button>
        <Button type="button" @click="confirmChange">{{ t('common.continue') }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
