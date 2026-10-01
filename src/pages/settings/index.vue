<script setup lang="ts">
import { Activity, Database, Info, PanelLeft, Settings2 } from '@lucide/vue'
import { getVersion } from '@tauri-apps/api/app'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
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
import { requestAutomaticUpdateInstall } from '@/features/updater/install-intent'
import { colorMode } from '@/lib/color-mode'

import appIconUrl from '../../../src-tauri/icons/icon.png'
import AboutSettingsPanel from './about/index.vue'
import DataSettingsPanel from './data/index.vue'
import DiagnosticsSettingsPanel from './diagnostics/index.vue'
import GeneralSettingsPanel from './general/index.vue'
import type { NavigationItem, SettingsSection } from './model'
import TaskbarSettingsPanel from './taskbar/index.vue'

const SETTINGS_SECTION_EVENT = 'settings://select-section'
const sectionOrder: SettingsSection[] = ['general', 'taskbar', 'data', 'diagnostics', 'about']
const route = useRoute()
const initialSection = sectionOrder.find((section) => section === route.query.section) ?? 'general'
const activeSection = shallowRef<SettingsSection>(initialSection)
const applicationVersion = shallowRef('—')
const { t } = useI18n({ useScope: 'global' })
let unlistenSection: UnlistenFn | undefined

interface SettingsNavigation {
  section: SettingsSection
  automaticInstall: boolean
}

/** 只接受原生层约定的设置页导航负载。 */
function parseSettingsNavigation(value: unknown): SettingsNavigation | null {
  if (typeof value !== 'object' || value === null) return null
  const candidate = value as Partial<Record<keyof SettingsNavigation, unknown>>
  const section = sectionOrder.find((item) => item === candidate.section)
  if (!section || typeof candidate.automaticInstall !== 'boolean') return null
  return { section, automaticInstall: candidate.automaticInstall }
}

if (initialSection === 'about' && route.query.automaticInstall === 'true') {
  requestAutomaticUpdateInstall()
}

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

/** 响应原生托盘更新入口，在已打开的设置窗口中切换到“关于”页。 */
onMounted(async () => {
  try {
    unlistenSection = await listen<unknown>(SETTINGS_SECTION_EVENT, ({ payload }) => {
      const navigation = parseSettingsNavigation(payload)
      if (!navigation) return
      selectSection(navigation.section)
      if (navigation.automaticInstall) requestAutomaticUpdateInstall()
    })
  } catch (error) {
    reportBackgroundFailure('监听设置页定位事件失败', error)
  }
})

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

onUnmounted(() => unlistenSection?.())
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
                <span class="truncate font-semibold">MuseTune</span>
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

      <div class="settings-scrollbar flex-1 overflow-y-auto">
        <KeepAlive :max="navigationItems.length">
          <component
            :is="activePanel"
            v-bind="activeSection === 'about' ? { applicationVersion } : {}"
            class="w-full"
          />
        </KeepAlive>
      </div>
    </SidebarInset>
  </SidebarProvider>
  <Toaster :theme="toasterTheme" position="bottom-right" rich-colors />
</template>
