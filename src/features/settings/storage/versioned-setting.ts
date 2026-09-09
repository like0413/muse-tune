import { settingsStore } from '../store'

interface VersionedSetting<T> {
  version: number
  value: T
}

interface VersionedSettingOptions<T> {
  key: string
  version: number
  defaultValue: T
  normalize: (value: unknown) => T
}

/** 判断存储值是否为指定版本的设置封装。 */
function isCurrentVersionedSetting(
  value: unknown,
  version: number,
): value is VersionedSetting<unknown> {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Partial<VersionedSetting<unknown>>
  return candidate.version === version && 'value' in candidate
}

/** 读取单项版本化设置；裸数据或版本不一致时只重置当前设置项。 */
export async function getVersionedSetting<T>(options: VersionedSettingOptions<T>): Promise<T> {
  const stored = await settingsStore.get<unknown>(options.key)
  if (isCurrentVersionedSetting(stored, options.version)) {
    return options.normalize(stored.value)
  }

  const fallback = options.normalize(options.defaultValue)
  await settingsStore.set(options.key, {
    version: options.version,
    value: fallback,
  } satisfies VersionedSetting<T>)
  return fallback
}

/** 规范并保存单项版本化设置，确保值和版本在同一个存储键内更新。 */
export async function setVersionedSetting<T>(
  options: VersionedSettingOptions<T>,
  value: unknown,
): Promise<T> {
  const normalized = options.normalize(value)
  await settingsStore.set(options.key, {
    version: options.version,
    value: normalized,
  } satisfies VersionedSetting<T>)
  return normalized
}
