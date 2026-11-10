/** Direção de arte da Haru Live2D: paleta, sombreado cel e texturas derivadas da base VRoid. */
import { decodePng, encodePng, texturePng } from './haru-png.mjs'

export { texturePng }

/** Cores lidas nas texturas da Haru Live2D (`public/live2d/haru`): referência de paleta, não asset copiado. */
export const PALETTE = {
  ink: '#3d1b26',
  blazer: ['#4b4f5b', '#2b2e3b'],
  piping: ['#f1f1f4', '#a4a7b8'],
  shirt: ['#e4e5ea', '#9c9fb0'],
  hair: ['#7d7085', '#4a3f5c'],
  shoes: ['#1d1b22', '#08070b'],
  bow: ['#2a2530', '#0c0a10'],
}

const hex = (value) => [1, 3, 5].map((i) => Number.parseInt(value.slice(i, i + 2), 16) / 255)
// glTF guarda fatores lineares; a paleta foi lida em sRGB.
const linear = (value) => hex(value).map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))

const RIM_TINT = linear('#8f8ab8')

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
  [0, '#060b1c'],
  [0.12, '#10224f'],
  [0.3, '#1f58a8'],
  [0.5, '#3a9ae6'],
  [0.72, '#8fd8ff'],
  [1, '#f4fbff'],
]

/**
 * Luminância da base esticada para a faixa lavanda-acinzentada da Haru, em rampa curta: o
 * contraste da pintura VRoid virava dezenas de riscos, e a Live2D é quase lisa com um brilho.
 */
export const HAIR_STOPS = [
  [0.18, '#54486a'],
  [0.3, '#675a7c'],
  [0.4, '#766a8a'],
  [0.5, '#9a8ead'],
]

/** Mechas desenhadas à mão no estilo Live2D: poucos traços finos escuros e riscos claros. */
export function hairStrokes(color, u, v) {
  const wave = u + Math.sin(v * 7 + u * 13) * 0.004
  const dark = smooth(Math.cos(wave * Math.PI * 14) + Math.sin(wave * 57) * 0.08, 0.95, 0.995)
  const light = smooth(Math.cos((wave + 0.031) * Math.PI * 34), 0.985, 1) * smooth(v, 0.18, 0.3)
  const ring = Math.exp(-(((v - 0.27 - Math.sin(u * 19) * 0.012) / 0.028) ** 2)) * 0.55
  return mix(mix(color, [70, 55, 88], dark * 0.25), [226, 220, 238], Math.min(1, light * 0.35 + ring * 0.6))
}

/** Listras onduladas em diagonal: azul-claro, marinho e prata, como o lenço da Haru. */
export function scarfColor(u, v) {
  const s = ((((u * 1.1 + v * 1.6 + Math.sin(v * 6 + u * 3) * 0.06) * 1.4) % 1) + 1) % 1
  const band =
    s < 0.42
      ? '#5cc6f4'
      : s < 0.52
        ? '#f2f5fa'
        : s < 0.66
          ? '#3a8fd4'
          : s < 0.78
            ? '#1d3f9a'
            : s < 0.86
              ? '#cfd6e2'
              : '#5cc6f4'
  return [...hex(band).map((c) => c * 255), 255]
}

/** Tecido grafite com trama muito sutil; o volume vem do cel, não de ruído. */
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
  return [value * 0.92, value * 0.88, value, 255]
}
