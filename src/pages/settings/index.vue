<script setup lang="ts">
import { Info, Music2, Palette, PanelLeft, Settings2 } from '@lucide/vue'
import type { Component } from 'vue'
import { computed, shallowRef } from 'vue'

import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
} from '@/components/ui/sidebar'

import AboutSettingsPanel from './about/AboutSettingsPanel.vue'
import AppearanceSettingsPanel from './appearance/AppearanceSettingsPanel.vue'
import GeneralSettingsPanel from './general/GeneralSettingsPanel.vue'
import type { NavigationItem, SettingsSection } from './model'
import { SETTINGS_SECTION_META } from './model'
import TaskbarSettingsPanel from './taskbar/TaskbarSettingsPanel.vue'

const activeSection = shallowRef<SettingsSection>('general')

const navigationItems: Array<NavigationItem> = [
  { id: 'general', label: '常规', icon: Settings2 },
  { id: 'taskbar', label: '任务栏', icon: PanelLeft },
  { id: 'appearance', label: '外观', icon: Palette },
  { id: 'about', label: '关于', icon: Info },
]

const panels: Record<SettingsSection, Component> = {
  general: GeneralSettingsPanel,
  taskbar: TaskbarSettingsPanel,
  appearance: AppearanceSettingsPanel,
  about: AboutSettingsPanel,
}

const activePanel = computed(() => panels[activeSection.value])
const activeSectionMeta = computed(() => SETTINGS_SECTION_META[activeSection.value])

function selectSection(section: SettingsSection) {
  activeSection.value = section
}
</script>

<template>
  <SidebarProvider class="h-full">
    <Sidebar collapsible="none" class="w-52 border-r">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" class="pointer-events-none">
              <div
                class="icon-tone-pink-500 flex aspect-square size-8 items-center justify-center rounded-lg border"
              >
                <Music2 />
              </div>
              <span class="grid flex-1 text-left leading-tight">
                <span class="truncate font-semibold">Muse Tune</span>
                <span class="text-muted-foreground truncate text-xs">设置</span>
              </span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem v-for="item in navigationItems" :key="item.id">
                <SidebarMenuButton
                  :is-active="activeSection === item.id"
                  @click="selectSection(item.id)"
                >
                  <component :is="item.icon" :class="item.iconClass" />
                  <span>{{ item.label }}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>

    <SidebarInset>
      <header class="flex h-16 shrink-0 items-center px-6">
        <div class="grid gap-0.5">
          <h1 class="text-base font-semibold">{{ activeSectionMeta.title }}</h1>
          <p class="text-muted-foreground text-sm">{{ activeSectionMeta.description }}</p>
        </div>
      </header>
      <Separator />

      <div class="flex-1 overflow-y-auto p-4">
        <KeepAlive>
          <component :is="activePanel" class="w-full" />
        </KeepAlive>
      </div>
    </SidebarInset>
  </SidebarProvider>
</template>
