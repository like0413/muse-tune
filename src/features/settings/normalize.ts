import { clamp } from 'es-toolkit'

/**
 * 把外部值收敛为区间内的整数。
 *
 * 只接受有限数；非法值返回 `undefined`，由各设置模块决定自己的回退默认值，
 * 使「有限数 + 四舍五入 + 钳制」这条规则只在一处维护。
 */
export function normalizeIntegerInRange(
  value: unknown,
  min: number,
  max: number,
): number | undefined {
  if (typeof value !== 'number' || !Number.isFinite(value)) return undefined
  return Math.round(clamp(value, min, max))
}
