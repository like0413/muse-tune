import {
  createColor,
  getColor,
  getSwatches,
  type Color,
  type ExtractionOptions,
  type SwatchMap,
  type SwatchRole,
} from 'colorthief'
import { maxBy, minBy } from 'es-toolkit'

import { TASKBAR_THEME_PRESET_COLORS } from '@/features/settings/theme-color'

const MINIMUM_DOMINANT_CHROMA = 0.05
const MINIMUM_MUTED_CHROMA = 0.01
const COVER_COLOR_FALLBACK = TASKBAR_THEME_PRESET_COLORS[10]

const EXTRACTION_OPTIONS = {
  colorSpace: 'oklch',
  quality: 8,
  ignoreWhite: true,
  minSaturation: 0,
  gamut: 'srgb',
} satisfies ExtractionOptions

const PRESET_COLORS = TASKBAR_THEME_PRESET_COLORS.map((hex) => {
  const value = Number.parseInt(hex.slice(1), 16)
  const color = createColor((value >> 16) & 0xff, (value >> 8) & 0xff, value & 0xff, 0)
  return { hex, hue: color.oklch().h }
})
const VIBRANT_SWATCH_ROLES: SwatchRole[] = ['Vibrant', 'LightVibrant', 'DarkVibrant']
const MUTED_SWATCH_ROLES: SwatchRole[] = ['Muted', 'LightMuted', 'DarkMuted']

/** 加载并解码封面，避免 Color Thief 读取尚未就绪的图片。 */
async function loadCoverImage(source: string): Promise<HTMLImageElement> {
  const image = new Image()
  image.decoding = 'async'
  image.src = source
  await image.decode()
  return image
}

/** 计算两个色相在色环上的最短距离。 */
function hueDistance(first: number, second: number): number {
  const difference = Math.abs(first - second)
  return Math.min(difference, 360 - difference)
}

/** 读取有足够色度的色相，避免将纯黑白灰的无意义色相映射到色板。 */
function getUsableHue(color: Color | null | undefined, minimumChroma: number): number | null {
  if (!color) return null
  const { c, h } = color.oklch()
  return c >= minimumChroma && Number.isFinite(h) ? h : null
}

/** 从同类语义色中选择覆盖像素最多的颜色，避免小面积高饱和点缀抢占主色。 */
function getSwatchHue(
  swatches: SwatchMap,
  roles: SwatchRole[],
  minimumChroma: number,
): number | null {
  const color = maxBy(
    roles
      .map((role) => swatches[role]?.color)
      .filter((candidate): candidate is Color => getUsableHue(candidate, minimumChroma) !== null),
    (candidate) => candidate.population,
  )
  return getUsableHue(color, minimumChroma)
}

/** 将封面主色相映射到最接近的固定明亮主题色。 */
function matchPresetColor(hue: number): string {
  return minBy(PRESET_COLORS, (preset) => hueDistance(hue, preset.hue))?.hex ?? COVER_COLOR_FALLBACK
}

/** 识别封面主色系，并输出固定色板中最接近的明亮颜色。 */
export async function extractTaskbarCoverColor(source: string): Promise<string> {
  const image = await loadCoverImage(source)
  const dominantHue = getUsableHue(
    await getColor(image, EXTRACTION_OPTIONS),
    MINIMUM_DOMINANT_CHROMA,
  )
  if (dominantHue !== null) return matchPresetColor(dominantHue)

  // 主色接近黑白灰时，先找彩色点缀；没有彩色点缀再保留整体的冷暖倾向。
  const swatches = await getSwatches(image, EXTRACTION_OPTIONS)
  const hue =
    getSwatchHue(swatches, VIBRANT_SWATCH_ROLES, MINIMUM_DOMINANT_CHROMA) ??
    getSwatchHue(swatches, MUTED_SWATCH_ROLES, MINIMUM_MUTED_CHROMA)
  return hue === null ? COVER_COLOR_FALLBACK : matchPresetColor(hue)
}
