import { logDebug } from '@/features/logging'

const FADE_IN_SECONDS = 0.004
const FADE_OUT_SECONDS = 0.012

interface ActiveVoice {
  source: AudioBufferSourceNode
  gain: GainNode
}

/**
 * 预加载一组短交互音效，并在同一个音频上下文中按名称播放。
 *
 * 连续触发时仅保留极短交叉淡化：既不硬切波形产生爆点，也不让音效彼此叠加。
 */
export function useInteractionSounds<const SoundName extends string>(
  sources: Readonly<Record<SoundName, string>>,
) {
  const buffers = new Map<SoundName, AudioBuffer>()
  let context: AudioContext | undefined
  let activeVoice: ActiveVoice | undefined
  let loadController: AbortController | undefined
  let playRevision = 0
  let disposed = false

  /** 在窗口挂载后提前读取并解码本地音效，避免首次交互才开始加载。 */
  onMounted(() => {
    context = new AudioContext()
    loadController = new AbortController()
    const entries = Object.entries(sources) as [SoundName, string][]
    for (const [name, sourceUrl] of entries) {
      void loadSound(name, sourceUrl)
    }
  })

  /** 独立加载单个音效，避免一个资源失败影响其他交互音。 */
  async function loadSound(name: SoundName, sourceUrl: string) {
    const audioContext = context
    const signal = loadController?.signal
    if (!audioContext || !signal) return
    try {
      const response = await fetch(sourceUrl, { signal })
      if (!response.ok) throw new Error(`HTTP ${response.status}`)
      const decoded = await audioContext.decodeAudioData(await response.arrayBuffer())
      if (!disposed) buffers.set(name, decoded)
    } catch (error) {
      if (!disposed) logDebug(`加载交互音效失败: ${name}`, error)
    }
  }

  /** 淡出当前声音，并允许下一声立即进入。 */
  function fadeOutActiveVoice(now: number) {
    const voice = activeVoice
    if (!voice) return
    voice.gain.gain.cancelAndHoldAtTime(now)
    voice.gain.gain.linearRampToValueAtTime(0, now + FADE_OUT_SECONDS)
    voice.source.stop(now + FADE_OUT_SECONDS)
    activeVoice = undefined
  }

  /** 创建一次不可复用的 AudioBufferSource，并以极短淡入消除起播爆点。 */
  function startVoice(audioContext: AudioContext, audioBuffer: AudioBuffer) {
    const now = audioContext.currentTime
    fadeOutActiveVoice(now)

    const source = audioContext.createBufferSource()
    const gain = audioContext.createGain()
    source.buffer = audioBuffer
    gain.gain.setValueAtTime(0, now)
    gain.gain.linearRampToValueAtTime(1, now + FADE_IN_SECONDS)
    source.connect(gain).connect(audioContext.destination)
    activeVoice = { source, gain }
    source.addEventListener(
      'ended',
      () => {
        if (activeVoice?.source === source) activeVoice = undefined
        source.disconnect()
        gain.disconnect()
      },
      { once: true },
    )
    source.start(now)
  }

  /** 播放指定交互音；恢复音频上下文期间只保留最新一次请求。 */
  function playInteractionSound(name: SoundName) {
    const audioContext = context
    const audioBuffer = buffers.get(name)
    if (!audioContext || !audioBuffer || disposed) return
    const revision = ++playRevision
    if (audioContext.state === 'running') {
      startVoice(audioContext, audioBuffer)
      return
    }
    void audioContext
      .resume()
      .then(() => {
        if (!disposed && revision === playRevision && audioContext.state === 'running') {
          startVoice(audioContext, audioBuffer)
        }
      })
      .catch((error) => logDebug('恢复交互音效失败', error))
  }

  onUnmounted(() => {
    disposed = true
    playRevision += 1
    loadController?.abort()
    buffers.clear()
    if (context) {
      fadeOutActiveVoice(context.currentTime)
      void context.close().catch(() => undefined)
    }
    context = undefined
  })

  return { playInteractionSound }
}
