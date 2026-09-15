import type { Component } from 'vue'

export type SettingsSection = 'general' | 'taskbar' | 'data' | 'diagnostics' | 'about'

export type NavigationItem = {
  id: SettingsSection
  label: string
  icon: Component
  iconClass?: string
}
