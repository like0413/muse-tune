/** 应用默认品牌色；Hex 同时满足颜色选择器、持久化设置和原生 IPC 的格式约束。 */
export const DEFAULT_BRAND_COLOR_HEX = '#1677ff'

/** 自定义歌词配色的默认未播放色；主题配色模式仍由明暗主题令牌决定。 */
export const DEFAULT_LYRICS_UNPLAYED_COLOR_HEX = '#adb1b3'

/** 任务栏主题与封面取色共同使用的候选色板。 */
export const TASKBAR_THEME_PRESET_COLORS = [
  '#ef4444',
  '#f97316',
  '#f59e0b',
  '#eab308',
  '#84cc16',
  '#22c55e',
  '#10b981',
  '#14b8a6',
  '#06b6d4',
  '#0ea5e9',
  '#3b82f6',
  '#6366f1',
  '#8b5cf6',
  '#ec4899',
] as const
