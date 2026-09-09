/**
 * 所有版本化设置的当前存储结构版本。
 * 仅在对应设置发生不兼容的字段、类型、单位或语义变化时递增。
 */
export const SETTINGS_SCHEMA_VERSIONS = {
  taskbar: {
    audioSpectrum: 1,
    backgroundTransparency: 1,
  },
} as const
