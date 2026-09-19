import nativeDefaultsJson from '../native-defaults.json'

/**
 * 所有版本化设置的当前存储结构版本。
 * 仅在对应设置发生不兼容的字段、类型、单位或语义变化时递增。
 *
 * 版本号与前端默认值、原生启动期读取同属一份共享契约：原生侧需要自行判断
 * `taskbar.lyrics` 的存储版本是否过期（它读的是 `{version, value}` 包装里的原始值），
 * 因此版本号必须与默认值放在同一份文件中，避免两端各自维护。
 * 版本号是纯数字，JSON 导入不会损失类型，无需断言。
 */
export const SETTINGS_SCHEMA_VERSIONS = nativeDefaultsJson.versions
