<script setup lang="ts">
import { ArrowUpRight } from '@lucide/vue'

import {
  PROJECT_AUTHOR,
  PROJECT_AUTHOR_URL,
  PROJECT_CHANGELOG_URL,
  PROJECT_REPOSITORY_URL,
  PROJECT_WEBSITE_URL,
} from '@/features/project/metadata'
import { useProjectLinks } from '@/features/project/useProjectLinks'

import appIconUrl from '../../../../src-tauri/icons/icon.png'

defineProps<{ applicationVersion: string }>()

const { t } = useI18n({ useScope: 'global' })
const { openProjectUrl } = useProjectLinks()
</script>

<template>
  <section aria-labelledby="project-name" class="flex flex-col items-center gap-3 py-5 text-center">
    <img :src="appIconUrl" alt="" class="size-14 shrink-0 object-contain" />
    <div class="flex min-w-0 flex-col items-center gap-2">
      <div class="flex flex-col gap-1.5">
        <div class="flex flex-wrap items-baseline justify-center gap-2">
          <h2 id="project-name" class="text-xl font-semibold tracking-tight">MuseTune</h2>
          <span
            v-if="applicationVersion !== '—'"
            class="text-muted-foreground text-sm tabular-nums"
          >
            v{{ applicationVersion }}
          </span>
        </div>
        <p class="text-muted-foreground flex flex-wrap items-center justify-center gap-1 text-sm">
          {{ t('settings.about.project.createdBy') }}
          <a
            :href="PROJECT_AUTHOR_URL"
            class="inline-flex items-center gap-1 underline-offset-4 hover:underline"
            @click.prevent="openProjectUrl(PROJECT_AUTHOR_URL)"
          >
            {{ PROJECT_AUTHOR }}
          </a>
          <span aria-hidden="true">·</span>
          {{ t('settings.about.project.description') }}
        </p>
      </div>
      <div class="flex flex-wrap items-center justify-center gap-5 text-sm">
        <a
          :href="PROJECT_WEBSITE_URL"
          class="inline-flex items-center gap-1 underline-offset-4 hover:underline"
          @click.prevent="openProjectUrl(PROJECT_WEBSITE_URL)"
        >
          {{ t('settings.about.project.website')
          }}<ArrowUpRight class="text-muted-foreground size-3.5" aria-hidden="true" />
        </a>
        <a
          :href="PROJECT_REPOSITORY_URL"
          class="inline-flex items-center gap-1 underline-offset-4 hover:underline"
          @click.prevent="openProjectUrl(PROJECT_REPOSITORY_URL)"
        >
          GitHub<ArrowUpRight class="text-muted-foreground size-3.5" aria-hidden="true" />
        </a>
        <a
          :href="PROJECT_CHANGELOG_URL"
          class="inline-flex items-center gap-1 underline-offset-4 hover:underline"
          @click.prevent="openProjectUrl(PROJECT_CHANGELOG_URL)"
        >
          {{ t('settings.about.project.changelog')
          }}<ArrowUpRight class="text-muted-foreground size-3.5" aria-hidden="true" />
        </a>
      </div>
    </div>
  </section>
</template>
