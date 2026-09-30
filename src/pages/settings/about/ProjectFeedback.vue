<script setup lang="ts">
import { ArrowUpRight, MessageCircle } from '@lucide/vue'
import { useClipboard } from '@vueuse/core'
import { toast } from 'vue-sonner'

import { Item, ItemHeader, ItemFooter, ItemTitle } from '@/components/ui/item'
import {
  PROJECT_FEEDBACK_URL,
  PROJECT_ISSUES_URL,
  PROJECT_QQ_GROUP,
} from '@/features/project/metadata'
import { useProjectLinks } from '@/features/project/useProjectLinks'

const { t } = useI18n({ useScope: 'global' })
const { openProjectUrl } = useProjectLinks()
const { copy, copied, isSupported } = useClipboard({ source: PROJECT_QQ_GROUP })

/** 仅在用户点击时复制群号；失败时明确提示，不误报成功。 */
async function copyGroupNumber() {
  if (!isSupported.value) {
    toast.error(t('settings.about.feedback.copyFailed'))
    return
  }
  try {
    await copy()
  } catch {
    toast.error(t('settings.about.feedback.copyFailed'))
  }
}
</script>

<template>
  <Item role="region" aria-labelledby="project-feedback">
    <ItemHeader>
      <ItemTitle id="project-feedback">
        <MessageCircle class="size-4 text-violet-500" />
        {{ t('settings.about.feedback.title') }}
      </ItemTitle>
    </ItemHeader>
    <ItemFooter class="flex-wrap justify-start gap-x-6 gap-y-3">
      <div class="flex flex-wrap items-center gap-2">
        <span class="text-muted-foreground">{{ t('settings.about.feedback.qq') }}</span>
        <button
          type="button"
          class="focus-visible:outline-ring inline-flex items-center gap-1 underline-offset-4 hover:underline"
          :aria-label="t('settings.about.feedback.copy')"
          @click="copyGroupNumber"
        >
          {{ PROJECT_QQ_GROUP }}
        </button>
        <span v-if="copied" role="status" class="text-muted-foreground text-xs">{{
          t('settings.about.feedback.copied')
        }}</span>
      </div>
      <a
        :href="PROJECT_FEEDBACK_URL"
        class="text-primary inline-flex items-center gap-1 underline-offset-4 hover:underline"
        @click.prevent="openProjectUrl(PROJECT_FEEDBACK_URL)"
      >
        {{ t('settings.about.feedback.form') }}<ArrowUpRight class="size-3.5" aria-hidden="true" />
      </a>
      <a
        :href="PROJECT_ISSUES_URL"
        class="inline-flex items-center gap-1 underline-offset-4 hover:underline"
        @click.prevent="openProjectUrl(PROJECT_ISSUES_URL)"
      >
        GitHub Issues<ArrowUpRight class="text-muted-foreground size-3.5" aria-hidden="true" />
      </a>
    </ItemFooter>
  </Item>
</template>
