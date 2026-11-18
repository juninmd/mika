/** Direção de arte da Haru Live2D: paleta, sombreado cel e texturas derivadas da base VRoid. */
import { decodePng, encodePng, texturePng } from './haru-png.mjs'

export { texturePng }

/**
 * Paleta quente (mel, vinho e champanhe): a Mika Aurora é fria (preto-azulado, ciano e pastel),
 * e a Haru lavanda com blazer grafite lia como uma variação dela.
 */
export const PALETTE = {
  ink: '#4a1e1c',
  blazer: ['#7a2236', '#46101f'],
  piping: ['#f3dfb0', '#b98d55'],
  shirt: ['#ffffff', '#b4b3c9'],
  hair: ['#f0c48e', '#b26f4c'],
  shoes: ['#3a1d1c', '#160909'],
  bow: ['#d9b26a', '#8a5d2c'],
}

const hex = (value) => [1, 3, 5].map((i) => Number.parseInt(value.slice(i, i + 2), 16) / 255)
// glTF guarda fatores lineares; a paleta foi lida em sRGB.
const linear = (value) => hex(value).map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))

const RIM_TINT = linear('#e8b49a')

/**
 * Anime shadows shift toward blue-violet and keep some light; darkening the same hue turned navy and
 * black cloth into flat silhouettes with no readable folds.
 */
function animeShade(lit, shade) {
  const cool = [0.9, 0.92, 1.12]
  return shade.map((c, i) => Math.min(1, Math.max(c, lit[i] * 0.42) * cool[i] + 0.004 * cool[i]))
}

/** MToon com fronteira de sombra dura e contorno bordô: o traço que define a leitura anime. */
export function toonMaterial(name, [lit, shade], { outline = 0.0009, texture, rim = 0.18 } = {}) {
  return {
    name,
    doubleSided: true,
    pbrMetallicRoughness: {
      baseColorFactor: [...linear(lit), 1],
      metallicFactor: 0,
      roughnessFactor: 1,
      ...(texture ? { baseColorTexture: texture } : {}),
    },
    extensions: {
      KHR_materials_unlit: {},
      VRMC_materials_mtoon: {
        specVersion: '1.0',
        shadeColorFactor: animeShade(linear(lit), linear(shade)),
        ...(texture ? { shadeMultiplyTexture: texture } : {}),
        shadingToonyFactor: 0.9,
        shadingShiftFactor: -0.1,
        giEqualizationFactor: 0.85,
        // A cool rim of fixed strength: scaled by the lit colour it vanished on navy and black cloth.
        parametricRimColorFactor: linear(lit).map((c, i) => (c * 0.5 + RIM_TINT[i]) * rim),
        parametricRimFresnelPowerFactor: 3.5,
        parametricRimLiftFactor: 0.04,
        rimLightingMixFactor: 1,
        outlineWidthMode: outline ? 'worldCoordinates' : 'none',
        outlineWidthFactor: outline,
        outlineColorFactor: linear(PALETTE.ink),
        outlineLightingMixFactor: 0.5,
      },
    },
  }
}

/** Reaplica o mesmo acabamento cel a um material existente da base, mantendo suas texturas. */
export function inkOutline(material, width) {
  const toon = material.extensions.VRMC_materials_mtoon
  toon.outlineWidthMode = 'worldCoordinates'
  toon.outlineWidthFactor = width
  toon.outlineColorFactor = linear(PALETTE.ink)
  toon.outlineLightingMixFactor = 0.5
  toon.shadingToonyFactor = 0.93
  toon.shadingShiftFactor = -0.1
}

const mix = (a, b, t) => a.map((v, i) => v + (b[i] - v) * t)
const smooth = (t, a, b) => {
  const x = Math.max(0, Math.min(1, (t - a) / (b - a)))
  return x * x * (3 - 2 * x)
}

function ramp(stops, t) {
  const i = stops.findIndex(([at]) => at >= t)
  if (i <= 0) return stops[Math.max(0, i === -1 ? stops.length - 1 : 0)][1]
  const [a, ca] = stops[i - 1]
  const [b, cb] = stops[i]
  return mix(ca, cb, (t - a) / (b - a))
}

/** Troca a cor pela luminância: preserva desenho, reflexos e gradientes da pintura original. */
export function recolorPng(png, stops, detail) {
  const { width, height, rgba } = decodePng(png)
  const rgbStops = stops.map(([at, color]) => [at, hex(color).map((c) => c * 255)])
  for (let i = 0; i < width * height; i++) {
    const o = i * 4
    const lum = (0.299 * rgba[o] + 0.587 * rgba[o + 1] + 0.114 * rgba[o + 2]) / 255
    let color = ramp(rgbStops, lum)
    if (detail) color = detail(color, (i % width) / width, Math.floor(i / width) / height)
    rgba.set(
      color.map((c) => Math.round(Math.max(0, Math.min(255, c)))),
      o,
    )
  }
  return encodePng(width, height, rgba)
}

export const IRIS_STOPS = [
  [0, '#1c0806'],
  [0.12, '#4a1410'],
  [0.3, '#a2381f'],
  [0.5, '#e07a2c'],
  [0.72, '#ffc768'],
  [1, '#fff8e6'],
]

/**
 * Luminância da base esticada para um loiro mel rosado, em rampa curta: o contraste da pintura
 * VRoid virava dezenas de riscos, e o visual anime é quase liso com um brilho.
 */
export const HAIR_STOPS = [
  [0.18, '#c4836c'],
  [0.3, '#e0a888'],
  [0.4, '#f0c39f'],
  [0.5, '#fde0c2'],
]

/** Mechas desenhadas à mão no estilo Live2D: poucos traços finos escuros e riscos claros. */
export function hairStrokes(color, u, v) {
  const wave = u + Math.sin(v * 7 + u * 13) * 0.004
  const dark = smooth(Math.cos(wave * Math.PI * 14) + Math.sin(wave * 57) * 0.08, 0.95, 0.995)
  const light = smooth(Math.cos((wave + 0.031) * Math.PI * 34), 0.985, 1) * smooth(v, 0.18, 0.3)
  const ring = Math.exp(-(((v - 0.27 - Math.sin(u * 19) * 0.012) / 0.028) ** 2)) * 0.55
  return mix(mix(color, [150, 76, 58], dark * 0.25), [255, 240, 214], Math.min(1, light * 0.35 + ring * 0.6))
}

/** Listras onduladas em diagonal: marfim, dourado e vinho, combinando com o blazer. */
export function scarfColor(u, v) {
  const s = ((((u * 1.1 + v * 1.6 + Math.sin(v * 6 + u * 3) * 0.06) * 1.4) % 1) + 1) % 1
  const band =
    s < 0.42
      ? '#fbf1dc'
      : s < 0.52
        ? '#d9a84e'
        : s < 0.66
          ? '#f6e2b8'
          : s < 0.78
            ? '#8a2f3f'
            : s < 0.86
              ? '#d9a84e'
              : '#fbf1dc'
  return [...hex(band).map((c) => c * 255), 255]
}

/** Tecido com trama muito sutil; o volume vem do cel, não de ruído. */
export function clothColor(u, v) {
  const weave = Math.sin(u * Math.PI * 512) * Math.sin(v * Math.PI * 512) * 3
  return [247 + weave, 247 + weave, 250 + weave, 255]
}

/**
 * Matcap (view-space normal lookup, added over the lit colour) with a soft arc near the top: the
 * anime "angel ring" highlight that slides over the hair as the head turns.
 */
export function angelRingColor(u, v) {
  const nx = u * 2 - 1
  const ny = 1 - v * 2
  const band = Math.exp(-(((ny - 0.55) / 0.065) ** 2)) * Math.max(0, 1 - (nx / 0.78) ** 4)
  const streak = 0.75 + 0.25 * Math.sin(nx * 38)
  const value = 255 * band * streak
  return [value, value * 0.94, value * 0.84, 255]
}
