import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { DEFAULT_REDUCED_MOTION } from './defaults'
import { settingsStore } from './store'

const REDUCED_MOTION_KEY = 'application.reducedMotion'
const REDUCED_MOTION_CHANGED_EVENT = 'settings://reduced-motion-changed'

/** 读取应用级减少动态效果覆盖项。 */
export async function getReducedMotionOverride(): Promise<boolean> {
  const stored = await settingsStore.get<unknown>(REDUCED_MOTION_KEY)
  return typeof stored === 'boolean' ? stored : DEFAULT_REDUCED_MOTION
}

/** 保存应用级覆盖项，并广播给所有 WebView。 */
export async function setReducedMotionOverride(enabled: boolean): Promise<void> {
  await settingsStore.set(REDUCED_MOTION_KEY, enabled)
  await emit(REDUCED_MOTION_CHANGED_EVENT, enabled)
}

/** 监听应用级减少动态效果变化。 */
export async function listenReducedMotionOverrideChange(
  handler: (enabled: boolean) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(REDUCED_MOTION_CHANGED_EVENT, ({ payload }) => {
    if (typeof payload === 'boolean') handler(payload)
  })
}
