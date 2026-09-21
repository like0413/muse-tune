<script setup lang="ts">
import { Captions } from '@lucide/vue'

import { FieldGroup } from '@/components/ui/field'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Switch } from '@/components/ui/switch'
import { useLyricsDisplaySetting } from '@/features/settings/controllers/useLyricsDisplaySetting'

import LyricsLayoutSettings from './LyricsLayoutSettings.vue'
import LyricsSourceSettings from './LyricsSourceSettings.vue'
import LyricsTimingSettings from './LyricsTimingSettings.vue'

const { t } = useI18n({ useScope: 'global' })

const {
  selectedSettings,
  settingsSaving,
  updateSettings,
  previewTimingOffset,
  commitTimingOffset,
  previewFontSize,
  commitFontSize,
} = useLyricsDisplaySetting()
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-cyan-500">
      <Captions />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>{{ t('settings.taskbar.lyrics.title') }}</ItemTitle>
      <ItemDescription>{{ t('settings.taskbar.lyrics.description') }}</ItemDescription>
    </ItemContent>
    <template #actions>
      <Switch
        :model-value="selectedSettings.enabled"
        :disabled="settingsSaving"
        :aria-label="t('settings.taskbar.lyrics.enabled')"
        @update:model-value="updateSettings({ enabled: $event })"
      />
    </template>

    <template #content>
      <FieldGroup>
        <LyricsLayoutSettings
          :settings="selectedSettings"
          :saving="settingsSaving"
          @update-settings="updateSettings"
          @preview-font-size="previewFontSize"
          @commit-font-size="commitFontSize"
        />
        <LyricsTimingSettings
          :settings="selectedSettings"
          :saving="settingsSaving"
          @update-settings="updateSettings"
          @preview-timing-offset="previewTimingOffset"
          @commit-timing-offset="commitTimingOffset"
        />
        <LyricsSourceSettings
          :settings="selectedSettings"
          :saving="settingsSaving"
          @update-settings="updateSettings"
        />
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
