<script setup lang="ts">
import { MoonStar } from '@lucide/vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { colorMode } from '@/lib/color-mode'

const { t } = useI18n({ useScope: 'global' })

const selectedColorMode = colorMode.store
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-violet-500">
      <MoonStar />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.general.colorMode.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.general.colorMode.description') }}</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Tabs v-model="selectedColorMode">
        <TabsList>
          <TabsTrigger value="auto">{{ t('settings.general.colorMode.auto') }}</TabsTrigger>
          <TabsTrigger value="dark">{{ t('settings.general.colorMode.dark') }}</TabsTrigger>
          <TabsTrigger value="light">{{ t('settings.general.colorMode.light') }}</TabsTrigger>
        </TabsList>
      </Tabs>
    </ItemActions>
  </Item>
</template>
