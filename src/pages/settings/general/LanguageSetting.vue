<script setup lang="ts">
import { Languages } from '@lucide/vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifySettingSaveFailed } from '@/features/feedback/errors'
import { applyApplicationLocale } from '@/features/i18n'
import { isApplicationLocale, type ApplicationLocale } from '@/features/i18n/locales'
import { setApplicationLocale } from '@/features/i18n/settings'

const { locale, t } = useI18n({ useScope: 'global' })
const localeSaving = shallowRef(false)

/** 立即切换界面语言，并在保存失败时恢复原值。 */
async function selectLocale(value: string | number) {
  if (localeSaving.value || !isApplicationLocale(value) || value === locale.value) return
  const previousLocale = locale.value as ApplicationLocale
  applyApplicationLocale(value)
  localeSaving.value = true
  try {
    await setApplicationLocale(value)
  } catch (error) {
    applyApplicationLocale(previousLocale)
    notifySettingSaveFailed(t('settings.general.language.title'), error)
  } finally {
    localeSaving.value = false
  }
}
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-sky-500">
      <Languages />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.general.language.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.general.language.description') }}</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Tabs :model-value="locale" @update:model-value="selectLocale">
        <TabsList>
          <TabsTrigger value="zh-Hans" :disabled="localeSaving">简体中文</TabsTrigger>
          <TabsTrigger value="zh-Hant" :disabled="localeSaving">繁體中文</TabsTrigger>
          <TabsTrigger value="en" :disabled="localeSaving">English</TabsTrigger>
        </TabsList>
      </Tabs>
    </ItemActions>
  </Item>
</template>
