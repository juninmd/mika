/**
 * Anel de anjo do cabelo: o reflexo em faixa que o anime desenha no alto da cabeça e que fica parado em
 * relação à câmera — não escorrega quando ela vira o rosto. No MToon isso é o `matcap`: a cor vem da
 * normal no espaço da vista, sem UV. Textura procedural (faixa suave, janela horizontal, resto preto),
 * pixels puros e determinísticos como em `vrm-fabric.ts`.
 */
import * as THREE from 'three'

const SIZE = 128
/** Linha (0..1) e meia-altura da faixa; ajustados contra o render real do cabelo. */
const BAND_CENTER = 0.3
const BAND_HALF = 0.07
/** Quanto da largura o reflexo cobre antes de sumir nas laterais, onde a normal já vira de lado. */
const WINDOW = 0.78
/** Lavanda clara: um branco puro sobre cabelo roxo lê como mancha, não como brilho. */
const TINT = [0.82, 0.8, 0.96] as const
const PEAK = 0.9

export function shinePixels(): Uint8Array {
  const out = new Uint8Array(SIZE * SIZE * 4)
  for (let row = 0; row < SIZE; row++) {
    const v = (row + 0.5) / SIZE
    const band = Math.exp(-(((v - BAND_CENTER) / BAND_HALF) ** 2))
    for (let col = 0; col < SIZE; col++) {
      const u = (col + 0.5) / SIZE
      const across = Math.max(0, 1 - ((2 * u - 1) / WINDOW) ** 2)
      const level = PEAK * band * across * across
      const at = (row * SIZE + col) * 4
      out[at] = Math.round(255 * TINT[0] * level)
      out[at + 1] = Math.round(255 * TINT[1] * level)
      out[at + 2] = Math.round(255 * TINT[2] * level)
      out[at + 3] = 255
    }
  }
  return out
}

type ShineMaterial = THREE.Material & {
  isMToonMaterial?: boolean
  isOutline?: boolean
  matcapTexture?: THREE.Texture | null
  matcapFactor?: THREE.Color
}

/** Só o cabelo (`*_HAIR`), nunca o contorno. */
export function isHairMaterial(name: string): boolean {
  return /_HAIR$/i.test(name)
}

let shared: THREE.DataTexture | null = null

function shineTexture(): THREE.DataTexture {
  if (shared) return shared
  const texture = new THREE.DataTexture(shinePixels(), SIZE, SIZE, THREE.RGBAFormat)
  texture.colorSpace = THREE.SRGBColorSpace
  texture.magFilter = texture.minFilter = THREE.LinearFilter
  texture.generateMipmaps = false
  texture.needsUpdate = true
  shared = texture
  return texture
}

/** Intensidade do reflexo sobre o cabelo: acento, não faixa de néon. */
export const SHINE_STRENGTH = 1.6

export function applyHairShine(root: THREE.Object3D): void {
  root.traverse((object) => {
    const mesh = object as THREE.Mesh
    if (!mesh.isMesh) return
    for (const material of (Array.isArray(mesh.material) ? mesh.material : [mesh.material]) as ShineMaterial[]) {
      if (!material.isMToonMaterial || material.isOutline || material.matcapTexture) continue
      if (!isHairMaterial(material.name)) continue
      material.matcapTexture = shineTexture()
      material.matcapFactor?.setRGB(SHINE_STRENGTH, SHINE_STRENGTH, SHINE_STRENGTH)
      material.needsUpdate = true
    }
  })
}
