import { getColor, getSwatches, type Color, type ExtractionOptions } from 'colorthief'

type RgbColor = readonly [number, number, number]

interface CoverColorOptions {
  background: RgbColor
  fallback: string
}

const MINIMUM_GRAPHIC_CONTRAST_RATIO = 3
const CONTRAST_SEARCH_STEPS = 12
const ACHROMATIC_OKLCH_CHROMA = 0.05

const DOMINANT_COLOR_OPTIONS = {
  colorSpace: 'oklch',
  quality: 8,
  ignoreWhite: false,
  minSaturation: 0,
  gamut: 'srgb',
} satisfies ExtractionOptions

const VIBRANT_COLOR_OPTIONS = {
  ...DOMINANT_COLOR_OPTIONS,
  ignoreWhite: true,
  minSaturation: 0.25,
} satisfies ExtractionOptions

/** 加载封面图片，并等待浏览器完成解码后再交给 Color Thief。 */
async function loadCoverImage(source: string): Promise<HTMLImageElement> {
  const image = new Image()
  image.decoding = 'async'
  image.src = source
  await image.decode()
  return image
}

/** 按 WCAG 相对亮度公式计算单个 sRGB 分量。 */
function linearizeSrgb(component: number): number {
  const value = component / 255
  return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
}

/** 计算 RGB 颜色的相对亮度。 */
function relativeLuminance(color: RgbColor): number {
  return (
    0.2126 * linearizeSrgb(color[0]) +
    0.7152 * linearizeSrgb(color[1]) +
    0.0722 * linearizeSrgb(color[2])
  )
}

/** 判断候选色与任务栏背景是否达到非文本图形建议的 3:1 对比度。 */
function hasClearContrast(color: RgbColor, background: RgbColor): boolean {
  const colorLuminance = relativeLuminance(color)
  const backgroundLuminance = relativeLuminance(background)
  const lighter = Math.max(colorLuminance, backgroundLuminance)
  const darker = Math.min(colorLuminance, backgroundLuminance)
  return (lighter + 0.05) / (darker + 0.05) >= MINIMUM_GRAPHIC_CONTRAST_RATIO
}

/** 将 HSL 颜色转换为 RGB，供对比度修正时保持原主题色相。 */
function hslToRgb(hue: number, saturation: number, lightness: number): RgbColor {
  const chroma = (1 - Math.abs(2 * lightness - 1)) * saturation
  const hueSection = (((hue % 1) + 1) % 1) * 6
  const secondComponent = chroma * (1 - Math.abs((hueSection % 2) - 1))
  const [redBase, greenBase, blueBase] =
    hueSection < 1
      ? [chroma, secondComponent, 0]
      : hueSection < 2
        ? [secondComponent, chroma, 0]
        : hueSection < 3
          ? [0, chroma, secondComponent]
          : hueSection < 4
            ? [0, secondComponent, chroma]
            : hueSection < 5
              ? [secondComponent, 0, chroma]
              : [chroma, 0, secondComponent]
  const offset = lightness - chroma / 2
  return [
    Math.round((redBase + offset) * 255),
    Math.round((greenBase + offset) * 255),
    Math.round((blueBase + offset) * 255),
  ]
}

/**
 * 在 HSL 中只调整亮度，以最小改动满足对比度；不会因任务栏明暗改选其他色相。
 */
function ensureContrast(color: Color, background: RgbColor): RgbColor {
  const rgb = color.rgb()
  const original: RgbColor = [rgb.r, rgb.g, rgb.b]
  if (hasClearContrast(original, background)) return original

  const hsl = color.hsl()
  const hue = hsl.h / 360
  const saturation = hsl.s / 100
  const lightness = hsl.l / 100
  const darken = relativeLuminance(background) > relativeLuminance(original)
  let lower = darken ? 0 : lightness
  let upper = darken ? lightness : 1
  let result = hslToRgb(hue, saturation, darken ? lower : upper)

  for (let step = 0; step < CONTRAST_SEARCH_STEPS; step += 1) {
    const candidateLightness = (lower + upper) / 2
    const candidate = hslToRgb(hue, saturation, candidateLightness)
    if (hasClearContrast(candidate, background)) {
      result = candidate
      if (darken) lower = candidateLightness
      else upper = candidateLightness
    } else if (darken) {
      upper = candidateLightness
    } else {
      lower = candidateLightness
    }
  }

  return result
}

/** 把 RGB 颜色转换为 CSS 十六进制颜色。 */
function toHex(color: RgbColor): string {
  return `#${color.map((component) => component.toString(16).padStart(2, '0')).join('')}`
}

/** 优先返回统计主色；当主色接近黑白灰时，才从同一封面中寻找鲜艳色。 */
async function selectCoverColor(image: HTMLImageElement): Promise<Color | null> {
  const dominantColor = await getColor(image, DOMINANT_COLOR_OPTIONS)
  if (!dominantColor) return null
  if (dominantColor.oklch().c >= ACHROMATIC_OKLCH_CHROMA) return dominantColor

  const swatches = await getSwatches(image, VIBRANT_COLOR_OPTIONS)
  return swatches.Vibrant?.color ?? null
}

/** 提取适合任务栏进度条的封面色；没有可靠彩色候选时返回主题兜底色。 */
export async function extractTaskbarCoverColor(
  source: string,
  { background, fallback }: CoverColorOptions,
): Promise<string> {
  const themeColor = await selectCoverColor(await loadCoverImage(source))
  return themeColor ? toHex(ensureContrast(themeColor, background)) : fallback
}
