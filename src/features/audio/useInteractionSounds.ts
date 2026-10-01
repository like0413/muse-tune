import { logDebug } from '@/features/logging'

const FADE_IN_SECONDS = 0.004
const FADE_OUT_SECONDS = 0.012
const AUDIO_CONTEXT_IDLE_MS = 5_000

interface ActiveVoice {
  source: AudioBufferSourceNode
  gain: GainNode
}

/**
 * 按需加载一组短交互音效，并在同一个音频上下文中按名称播放。
 *
 * 连续触发时仅保留极短交叉淡化：既不硬切波形产生爆点，也不让音效彼此叠加。
 */
export function useInteractionSounds<const SoundName extends string>(
  sources: Readonly<Record<SoundName, string>>,
) {
  const buffers = new Map<SoundName, AudioBuffer>()
  const loadingBuffers = new Map<SoundName, Promise<AudioBuffer>>()
  let context: AudioContext | undefined
  let activeVoice: ActiveVoice | undefined
  let idleTimer: number | undefined
  let playRevision = 0
  let disposed = false

  /** 首次使用时才创建真实音频上下文，避免启动阶段就占用音频服务资源。 */
  function getAudioContext() {
    context ??= new AudioContext()
    return context
  }

  /** 每个短音效只读取并解码一次；并发触发会复用同一个加载任务。 */
  function loadSound(name: SoundName, audioContext: AudioContext): Promise<AudioBuffer> {
    const cached = buffers.get(name)
    if (cached) return Promise.resolve(cached)
    const loading = loadingBuffers.get(name)
    if (loading) return loading

    const sourceUrl = sources[name]
    const task = fetch(sourceUrl)
      .then((response) => {
        if (!response.ok) throw new Error(`HTTP ${response.status}`)
        return response.arrayBuffer()
      })
      .then((encoded) => audioContext.decodeAudioData(encoded))
      .then((decoded) => {
        if (!disposed) buffers.set(name, decoded)
        return decoded
      })
      .finally(() => loadingBuffers.delete(name))
    loadingBuffers.set(name, task)
    return task
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

  /** 空闲后关闭硬件音频上下文；下次交互按需重建，并允许 WebView2 回收音频资源。 */
  function scheduleContextRelease(audioContext: AudioContext) {
    if (idleTimer !== undefined) window.clearTimeout(idleTimer)
    idleTimer = window.setTimeout(() => {
      idleTimer = undefined
      if (context !== audioContext) return
      fadeOutActiveVoice(audioContext.currentTime)
      // AudioBuffer 可跨上下文复用；仅释放硬件资源，避免每次空闲后重新读取与解码。
      context = undefined
      void audioContext.close().catch(() => undefined)
    }, AUDIO_CONTEXT_IDLE_MS)
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
    if (disposed) return
    const revision = ++playRevision
    if (idleTimer !== undefined) {
      window.clearTimeout(idleTimer)
      idleTimer = undefined
    }
    const audioContext = getAudioContext()
    void Promise.all([
      loadSound(name, audioContext),
      audioContext.state === 'running' ? Promise.resolve() : audioContext.resume(),
    ])
      .then(([audioBuffer]) => {
        if (
          !disposed &&
          revision === playRevision &&
          context === audioContext &&
          audioContext.state === 'running'
        ) {
          startVoice(audioContext, audioBuffer)
        }
      })
      .catch((error) => logDebug(`播放交互音效失败: ${name}`, error))
      .finally(() => {
        if (!disposed && context === audioContext) scheduleContextRelease(audioContext)
      })
  }

  onUnmounted(() => {
    disposed = true
    playRevision += 1
    if (idleTimer !== undefined) window.clearTimeout(idleTimer)
    buffers.clear()
    loadingBuffers.clear()
    if (context) {
      fadeOutActiveVoice(context.currentTime)
      void context.close().catch(() => undefined)
    }
    context = undefined
  })

  return { playInteractionSound }
}
