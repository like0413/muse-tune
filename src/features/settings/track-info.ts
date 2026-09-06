import type { UnlistenFn } from '@tauri-apps/api/event'
import { emit, listen } from '@tauri-apps/api/event'

import { settingsStore } from './store'

const TASKBAR_TRACK_INFO_ALIGNMENTS = ['left', 'right'] as const
const TASKBAR_TRACK_INFO_ALIGNMENT_KEY = 'taskbar.trackInfo.alignment'
const TASKBAR_TRACK_INFO_ALIGNMENT_CHANGED_EVENT = 'settings://taskbar-track-info-alignment-changed'

export type TaskbarTrackInfoAlignment = (typeof TASKBAR_TRACK_INFO_ALIGNMENTS)[number]

export const DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT: TaskbarTrackInfoAlignment = 'left'

/** 判断外部值是否为受支持的歌曲信息对齐方式。 */
export function isTaskbarTrackInfoAlignment(value: unknown): value is TaskbarTrackInfoAlignment {
  return (
    typeof value === 'string' &&
    TASKBAR_TRACK_INFO_ALIGNMENTS.some((alignment) => alignment === value)
  )
}

/** 读取歌曲信息对齐方式，缺失或损坏时使用左对齐。 */
export async function getTaskbarTrackInfoAlignment(): Promise<TaskbarTrackInfoAlignment> {
  const alignment = await settingsStore.get<unknown>(TASKBAR_TRACK_INFO_ALIGNMENT_KEY)
  return isTaskbarTrackInfoAlignment(alignment) ? alignment : DEFAULT_TASKBAR_TRACK_INFO_ALIGNMENT
}

/** 持久化歌曲信息对齐方式，并通知全部任务栏窗口。 */
export async function setTaskbarTrackInfoAlignment(
  alignment: TaskbarTrackInfoAlignment,
): Promise<void> {
  if (!isTaskbarTrackInfoAlignment(alignment)) throw new Error('无效的歌曲信息对齐方式')

  await settingsStore.set(TASKBAR_TRACK_INFO_ALIGNMENT_KEY, alignment)
  await emit(TASKBAR_TRACK_INFO_ALIGNMENT_CHANGED_EVENT, alignment)
}

/** 监听歌曲信息对齐方式变化。 */
export async function listenTaskbarTrackInfoAlignmentChange(
  handler: (alignment: TaskbarTrackInfoAlignment) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(TASKBAR_TRACK_INFO_ALIGNMENT_CHANGED_EVENT, ({ payload }) => {
    if (isTaskbarTrackInfoAlignment(payload)) handler(payload)
  })
}
