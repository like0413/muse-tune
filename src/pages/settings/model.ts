import type { Component } from 'vue'

export type SettingsSection = 'general' | 'taskbar' | 'data' | 'diagnostics' | 'about'

export type NavigationItem = {
  id: SettingsSection
  label: string
  icon: Component
  iconClass?: string
}

export interface SettingsSectionMeta {
  title: string
  description: string
}

export const SETTINGS_SECTION_META: Record<SettingsSection, SettingsSectionMeta> = {
  general: {
    title: '常规',
    description: '管理应用的启动与运行方式',
  },
  taskbar: {
    title: '任务栏',
    description: '调整任务栏播放器的位置与显示内容',
  },
  data: {
    title: '数据',
    description: '管理歌词缓存、配置和运行日志',
  },
  diagnostics: {
    title: '诊断',
    description: '查看应用、任务栏、媒体会话与歌词的当前运行状态',
  },
  about: {
    title: '关于',
    description: '查看 Muse Tune 的版本与项目信息',
  },
}
