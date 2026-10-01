import type { InjectionKey } from 'vue'

/** 高频播放时钟只由实际绘制进度的叶子组件读取，避免带动任务栏布局重渲染。 */
interface TaskbarPlaybackClock {
  progress: Readonly<Ref<number>>
  lyricsPositionMs: Readonly<Ref<number>>
}

const playbackClockKey: InjectionKey<TaskbarPlaybackClock> = Symbol('taskbar-playback-clock')

/** 在任务栏容器中提供只读时钟，组件共享同一外推结果。 */
export function provideTaskbarPlaybackClock(clock: TaskbarPlaybackClock) {
  provide(playbackClockKey, clock)
}

/** 读取任务栏时钟；缺少提供者表示组件被放入错误的上下文。 */
export function useTaskbarPlaybackClock(): TaskbarPlaybackClock {
  const clock = inject(playbackClockKey)
  if (!clock) throw new Error('任务栏播放时钟未初始化')
  return clock
}
