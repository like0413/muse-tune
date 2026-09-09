import type { MediaPlayerPresentation } from './types'

export const kugouMusicPresentation: MediaPlayerPresentation = {
  player: 'kugou_music',
  label: '酷狗音乐',
  fallbackText: 'K',
  fallbackClass: 'bg-blue-600 text-white',
  // 新版酷狗会先发布过渡图片，再在数毫秒后发布真实封面。
  thumbnailUpdateDebounceMs: 50,
}
