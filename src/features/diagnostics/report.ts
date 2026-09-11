import type { DiagnosticsSnapshot } from './types'

/** 生成不包含歌曲文本、账号路径、进程号或歌词正文的诊断报告。 */
export function createSanitizedDiagnosticsReport(diagnostics: DiagnosticsSnapshot): string {
  const report = {
    collectedAt: new Date().toISOString(),
    application: {
      ...diagnostics.application,
      cacheDirectory: diagnostics.application.cacheDirectory ? '[available]' : null,
      logDirectory: diagnostics.application.logDirectory ? '[available]' : null,
    },
    taskbar: {
      ...diagnostics.taskbar,
      displayTarget: diagnostics.taskbar.displayTarget === 'all' ? 'all' : '[display-redacted]',
      displays: diagnostics.taskbar.displays.map((display) => ({
        ...display,
        id: '[redacted]',
      })),
    },
    media: {
      ...diagnostics.media,
      title: diagnostics.media.title ? '[redacted]' : null,
      artist: diagnostics.media.artist ? '[redacted]' : null,
      audioProcessId: diagnostics.media.audioProcessId ? '[redacted]' : null,
      runtimeError: sanitizeText(diagnostics.media.runtimeError),
      sessions: diagnostics.media.sessions.map((session) => ({
        ...session,
        title: session.title ? '[redacted]' : null,
        artist: session.artist ? '[redacted]' : null,
      })),
    },
    lyrics: {
      enabled: diagnostics.lyrics.enabled,
      status: diagnostics.lyrics.snapshot.status,
      source: diagnostics.lyrics.snapshot.source
        ? { ...diagnostics.lyrics.snapshot.source, songId: '[redacted]' }
        : null,
      precision: diagnostics.lyrics.snapshot.precision,
      lineCount: diagnostics.lyrics.snapshot.lineCount,
      errorReason: sanitizeText(diagnostics.lyrics.snapshot.errorReason),
      currentPlayer: diagnostics.lyrics.currentPlayer,
      resolutionMethod: diagnostics.lyrics.resolutionMethod,
      localCacheAvailable: diagnostics.lyrics.localCacheAvailable,
      resolverRunning: diagnostics.lyrics.resolverRunning,
      pendingResolution: diagnostics.lyrics.pendingResolution,
      resolutionDurationMs: diagnostics.lyrics.resolutionDurationMs,
      resolutionSteps: diagnostics.lyrics.resolutionSteps.map((step) => ({
        ...step,
        detail: sanitizeText(step.detail),
      })),
      cache: diagnostics.lyrics.cache,
      adapters: diagnostics.lyrics.adapters.map((adapter) => ({
        ...adapter,
        cachePath: adapter.cachePath ? '[available]' : null,
      })),
    },
    storage: {
      ...diagnostics.storage,
      settingsFile: diagnostics.storage.settingsFile ? '[available]' : null,
    },
    issues: diagnostics.issues.map((issue) => ({
      ...issue,
      message: sanitizeText(issue.message),
    })),
  }
  return JSON.stringify(report, null, 2)
}

function sanitizeText(value: string | null): string | null {
  if (!value) return value
  return value
    .replace(/https?:\/\/\S+/g, '[url]')
    .replace(/[A-Za-z]:\\[^"']*/g, '[path]')
    .replace(/\\\\[^"']*/g, '[network-path]')
}
