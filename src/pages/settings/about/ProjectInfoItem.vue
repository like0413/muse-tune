<script setup lang="ts">
import { ExternalLink, Globe2 } from '@lucide/vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import { siGithub } from 'simple-icons'

import { Button } from '@/components/ui/button'
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemFooter,
  ItemHeader,
  ItemTitle,
} from '@/components/ui/item'
import { getErrorMessage } from '@/features/feedback/errors'
import { PROJECT_ISSUES_URL, PROJECT_REPOSITORY_URL } from '@/features/project/metadata'

const { t } = useI18n({ useScope: 'global' })

const errorMessage = shallowRef<string | null>(null)

/** 使用系统浏览器打开经过能力白名单约束的项目链接。 */
async function openProjectUrl(url: string) {
  try {
    await openUrl(url)
    errorMessage.value = null
  } catch (error) {
    errorMessage.value = getErrorMessage(error, t('settings.about.project.openFailed'))
  }
}
</script>

<template>
  <Item>
    <ItemHeader>
      <ItemContent>
        <ItemTitle>
          <Globe2 class="size-4 text-violet-500" />
          {{ t('settings.about.project.title') }}
        </ItemTitle>
        <ItemDescription>{{ t('settings.about.project.description') }}</ItemDescription>
      </ItemContent>
      <ItemActions>
        <Button variant="outline" size="sm">
          <Globe2 data-icon="inline-start" />
          {{ t('settings.about.project.website') }}
        </Button>
        <Button variant="outline" size="sm" @click="openProjectUrl(PROJECT_REPOSITORY_URL)">
          <svg data-icon="inline-start" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <path :d="siGithub.path" />
          </svg>
          {{ t('settings.about.project.repository') }}
        </Button>
        <Button variant="outline" size="sm" @click="openProjectUrl(PROJECT_ISSUES_URL)">
          <ExternalLink data-icon="inline-start" />
          {{ t('settings.about.project.issues') }}
        </Button>
      </ItemActions>
    </ItemHeader>
    <ItemFooter v-if="errorMessage">
      <p class="text-destructive text-xs">{{ errorMessage }}</p>
    </ItemFooter>
  </Item>
</template>
