import { debug, error, warn } from '@tauri-apps/plugin-log'

/**
 * 前端日志落盘通道。
 *
 * 正式构建没有控制台窗口、也不打开 devtools，前端控制台输出没有任何落点，只留在内存里。
 * 排查线上问题时唯一能看到的就是应用日志文件，所以前端失败必须显式转发过去。
 *
 * 日志文件只保留 warn 与 error：`logDebug` 用于“预期内的失败”（离线、竞态、可选数据缺失），
 * 它同样不会进入文件，需要本地排查时才把 Rust 侧日志级别临时调低。
 *
 * 行内不再标注来源：落盘时 target 列会被改写为 `前端:函数名`，与后端模块路径同处一列。
 *
 * 这里只负责转发：写入本身失败不能再抛异常，否则这个用于排查的辅助路径会变成新的故障源
 * （例如权限未授予时让原本只是记录失败的 catch 块再次中断）。
 */

/** 把任意异常或载荷收敛成一行文本，避免写入 `[object Object]`。 */
function describe(detail: unknown): string {
  if (typeof detail === 'string') return detail
  if (detail instanceof Error) return detail.stack ?? `${detail.name}: ${detail.message}`
  try {
    return JSON.stringify(detail) ?? String(detail)
  } catch {
    return String(detail)
  }
}

function write(level: 'debug' | 'warn' | 'error', context: string, detail?: unknown): void {
  const message = detail === undefined ? context : `${context}: ${describe(detail)}`
  const send = level === 'error' ? error : level === 'warn' ? warn : debug
  void send(message).catch(() => {})
}

/** 真实故障：写入日志文件，是前台失败唯一的追溯途径。 */
export function logError(context: string, detail?: unknown): void {
  write('error', context, detail)
}

/** 需要保留痕迹但属于可恢复的问题，例如某条链路降级后仍在运行。 */
export function logWarn(context: string, detail?: unknown): void {
  write('warn', context, detail)
}

/** 预期内的失败（离线、竞态、可选数据缺失）：不占用日志文件。 */
export function logDebug(context: string, detail?: unknown): void {
  write('debug', context, detail)
}
