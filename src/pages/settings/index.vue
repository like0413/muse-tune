<script setup lang="ts">
import { Activity, Database, Info, PanelLeft, Settings2 } from '@lucide/vue'
import { getVersion } from '@tauri-apps/api/app'
import type { Component } from 'vue'
import { computed, onMounted, shallowRef } from 'vue'

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
import { Toaster } from '@/components/ui/sonner'
import { colorMode } from '@/lib/color-mode'

import appIconUrl from '../../../src-tauri/icons/icon.png'
import AboutSettingsPanel from './about/index.vue'
import DataSettingsPanel from './data/index.vue'
import DiagnosticsSettingsPanel from './diagnostics/index.vue'
import GeneralSettingsPanel from './general/index.vue'
import type { NavigationItem, SettingsSection } from './model'
import { SETTINGS_SECTION_META } from './model'
import TaskbarSettingsPanel from './taskbar/index.vue'

const activeSection = shallowRef<SettingsSection>('general')
const applicationVersion = shallowRef('—')

const navigationItems: Array<NavigationItem> = [
  { id: 'general', label: '常规', icon: Settings2 },
  { id: 'taskbar', label: '任务栏', icon: PanelLeft },
  { id: 'data', label: '数据', icon: Database },
  { id: 'diagnostics', label: '诊断', icon: Activity },
  { id: 'about', label: '关于', icon: Info },
]

const panels: Record<SettingsSection, Component> = {
  general: GeneralSettingsPanel,
  taskbar: TaskbarSettingsPanel,
  data: DataSettingsPanel,
  diagnostics: DiagnosticsSettingsPanel,
  about: AboutSettingsPanel,
}

const activePanel = computed(() => panels[activeSection.value])
const activeSectionMeta = computed(() => SETTINGS_SECTION_META[activeSection.value])
const toasterTheme = computed(() => (colorMode.value === 'dark' ? 'dark' : 'light'))

function selectSection(section: SettingsSection) {
  activeSection.value = section
}

/** 读取 Tauri 配置中的应用版本，供侧边栏品牌区展示。 */
onMounted(async () => {
  try {
    applicationVersion.value = await getVersion()
  } catch (error) {
    console.debug('读取应用版本失败', error)
  }
})
</script>

<template>
  <SidebarProvider class="h-full">
    <Sidebar collapsible="none" class="w-52 border-r">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" class="pointer-events-none">
              <div
                class="flex aspect-square size-8 items-center justify-center overflow-hidden rounded-lg"
              >
                <img :src="appIconUrl" alt="" class="size-full object-contain" />
              </div>
              <span class="grid flex-1 text-left leading-tight">
                <span class="truncate font-semibold">Muse Tune</span>
                <span class="text-muted-foreground truncate text-xs"
                  >v{{ applicationVersion }}</span
                >
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

      <div class="settings-scrollbar flex-1 overflow-y-auto p-4">
        <KeepAlive>
          <component :is="activePanel" class="w-full" />
        </KeepAlive>
      </div>
    </SidebarInset>
  </SidebarProvider>
  <Toaster :theme="toasterTheme" position="bottom-right" rich-colors />
</template>
