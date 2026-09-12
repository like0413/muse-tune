<script setup lang="ts">
import { ExternalLink, Globe2 } from '@lucide/vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import { siGithub } from 'simple-icons'
import { shallowRef } from 'vue'

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

const PROJECT_URL = 'https://github.com/like0413/muse-tune'
const ISSUES_URL = `${PROJECT_URL}/issues`

const errorMessage = shallowRef<string | null>(null)

/** 使用系统浏览器打开经过能力白名单约束的项目链接。 */
async function openProjectUrl(url: string) {
  try {
    await openUrl(url)
    errorMessage.value = null
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error)
  }
}
</script>

<template>
  <Item>
    <ItemHeader>
      <ItemContent>
        <ItemTitle>项目与许可</ItemTitle>
        <ItemDescription>开源项目 · MIT License</ItemDescription>
      </ItemContent>
      <ItemActions>
        <Button variant="outline" size="sm">
          <Globe2 data-icon="inline-start" />
          官网
        </Button>
        <Button variant="outline" size="sm" @click="openProjectUrl(PROJECT_URL)">
          <svg data-icon="inline-start" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <path :d="siGithub.path" />
          </svg>
          项目主页
        </Button>
        <Button variant="outline" size="sm" @click="openProjectUrl(ISSUES_URL)">
          <ExternalLink data-icon="inline-start" />
          问题反馈
        </Button>
      </ItemActions>
    </ItemHeader>
    <ItemFooter v-if="errorMessage">
      <p class="text-destructive text-xs">{{ errorMessage }}</p>
    </ItemFooter>
  </Item>
</template>
