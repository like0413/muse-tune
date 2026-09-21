<script setup lang="ts">
import { Activity, Database, Info, PanelLeft, Settings2 } from '@lucide/vue'
import { getVersion } from '@tauri-apps/api/app'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'

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
import { reportBackgroundFailure } from '@/features/feedback/errors'
import { colorMode } from '@/lib/color-mode'

import appIconUrl from '../../../src-tauri/icons/icon.png'
import AboutSettingsPanel from './about/index.vue'
import DataSettingsPanel from './data/index.vue'
import DiagnosticsSettingsPanel from './diagnostics/index.vue'
import GeneralSettingsPanel from './general/index.vue'
import type { NavigationItem, SettingsSection } from './model'
import TaskbarSettingsPanel from './taskbar/index.vue'

const activeSection = shallowRef<SettingsSection>('general')
const applicationVersion = shallowRef('—')
const { t } = useI18n({ useScope: 'global' })

const sectionPresentations = computed<
  Record<SettingsSection, { title: string; description: string; icon: Component }>
>(() => ({
  general: {
    title: t('settings.sections.general.title'),
    description: t('settings.sections.general.description'),
    icon: Settings2,
  },
  taskbar: {
    title: t('settings.sections.taskbar.title'),
    description: t('settings.sections.taskbar.description'),
    icon: PanelLeft,
  },
  data: {
    title: t('settings.sections.data.title'),
    description: t('settings.sections.data.description'),
    icon: Database,
  },
  diagnostics: {
    title: t('settings.sections.diagnostics.title'),
    description: t('settings.sections.diagnostics.description'),
    icon: Activity,
  },
  about: {
    title: t('settings.sections.about.title'),
    description: t('settings.sections.about.description'),
    icon: Info,
  },
}))

const sectionOrder: SettingsSection[] = ['general', 'taskbar', 'data', 'diagnostics', 'about']
const navigationItems = computed<Array<NavigationItem>>(() =>
  sectionOrder.map((id) => ({
    id,
    label: sectionPresentations.value[id].title,
    icon: sectionPresentations.value[id].icon,
  })),
)

const panels: Record<SettingsSection, Component> = {
  general: GeneralSettingsPanel,
  taskbar: TaskbarSettingsPanel,
  data: DataSettingsPanel,
  diagnostics: DiagnosticsSettingsPanel,
  about: AboutSettingsPanel,
}

const activePanel = computed(() => panels[activeSection.value])
const activeSectionMeta = computed(() => sectionPresentations.value[activeSection.value])
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

/** 首屏挂载与页面资源加载完成后，再显示预先隐藏的原生设置窗口。 */
onMounted(async () => {
  try {
    await nextTick()
    if (document.readyState !== 'complete') {
      await new Promise<void>((resolve) =>
        window.addEventListener('load', () => resolve(), { once: true }),
      )
    }
    await document.fonts.ready
    const settingsWindow = getCurrentWebviewWindow()
    await settingsWindow.show()
    await settingsWindow.setFocus()
  } catch (error) {
    reportBackgroundFailure('显示设置窗口失败', error)
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
        <KeepAlive :max="navigationItems.length">
          <component :is="activePanel" class="w-full" />
        </KeepAlive>
      </div>
    </SidebarInset>
  </SidebarProvider>
  <Toaster :theme="toasterTheme" position="bottom-right" rich-colors />
</template>
