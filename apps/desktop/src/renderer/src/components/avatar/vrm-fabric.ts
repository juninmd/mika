/**
 * Detalhe de tecido procedural para as roupas do VRM que vêm sem textura (`Renewal_*`: cor chapada).
 * Cor chapada + sombra de MToon lê como plástico; um mapa em tons de cinza quase branco, multiplicado
 * pela cor autoral, devolve trama, canelado e pregas sem mudar a paleta. Pixels puros (sem canvas, sem
 * alocação por quadro): testáveis e determinísticos. Cada padrão é periódico, então repete sem emenda.
 */
import * as THREE from 'three'

export type FabricKind = 'knit' | 'twill' | 'rib' | 'pleat'

/** Lado do mosaico (px) e quantas vezes ele repete na UV — a UV de cada peça é 0..1 (a saia, 0..6). */
const FABRIC: Record<FabricKind, { size: number; repeat: readonly [number, number] }> = {
  knit: { size: 128, repeat: [6, 6] },
  twill: { size: 64, repeat: [4, 4] },
  rib: { size: 64, repeat: [2, 1] },
  pleat: { size: 128, repeat: [4, 1] },
}

const TAU = Math.PI * 2
const smoothstep = (t: number) => t * t * (3 - 2 * t)

/** Brilho 0..1 do padrão em (x, y) ∈ [0, 1): periódico em ambos os eixos. */
function shade(kind: FabricKind, x: number, y: number): number {
  switch (kind) {
    case 'knit': {
      // Losangos de ponto de malha: 8 por mosaico, de 0,94 a 1.
      const weave = Math.sin(TAU * 8 * (x + y)) * Math.sin(TAU * 8 * (x - y))
      return 0.94 + 0.06 * (0.5 + 0.5 * weave)
    }
    case 'twill':
      // Sarja: diagonal fina, quase imperceptível parada e viva quando a luz gira.
      return 0.95 + 0.05 * (0.5 + 0.5 * Math.sin(TAU * 5 * (x + y)))
    case 'rib':
      // Canelado: 10 fios verticais por mosaico.
      return 0.9 + 0.1 * (0.5 + 0.5 * Math.cos(TAU * 10 * x))
    case 'pleat': {
      // Uma prega por mosaico: vinco escuro numa ponta, crista clara no meio.
      const fold = Math.abs(2 * x - 1)
      return 0.8 + 0.2 * smoothstep(fold)
    }
  }
}

/** RGBA8 do mosaico. Cinza neutro: não tinge a cor autoral, só a modula. */
export function fabricPixels(kind: FabricKind): Uint8Array {
  const { size } = FABRIC[kind]
  const out = new Uint8Array(size * size * 4)
  for (let row = 0; row < size; row++) {
    for (let col = 0; col < size; col++) {
      const value = Math.round(255 * shade(kind, col / size, row / size))
      const at = (row * size + col) * 4
      out[at] = out[at + 1] = out[at + 2] = value
      out[at + 3] = 255
    }
  }
  return out
}

/** Qual tecido cada peça leva, pelo nome do material; undefined deixa a peça como está. */
export function fabricKindFor(name: string): FabricKind | undefined {
  if (/Cuff_Rib/i.test(name)) return 'rib'
  if (/arcade_Skirt/i.test(name)) return 'pleat'
  if (/Sleeves|Crewneck|Sock$/i.test(name)) return 'knit'
  if (/Jacket|Inner/i.test(name)) return 'twill'
  return undefined
}

const cache = new Map<FabricKind, THREE.DataTexture>()

function fabricTexture(kind: FabricKind): THREE.DataTexture {
  const cached = cache.get(kind)
  if (cached) return cached
  const { size, repeat } = FABRIC[kind]
  const texture = new THREE.DataTexture(fabricPixels(kind), size, size, THREE.RGBAFormat)
  texture.wrapS = texture.wrapT = THREE.RepeatWrapping
  texture.repeat.set(repeat[0], repeat[1])
  texture.colorSpace = THREE.SRGBColorSpace
  texture.magFilter = THREE.LinearFilter
  texture.minFilter = THREE.LinearMipmapLinearFilter
  texture.generateMipmaps = true
  texture.needsUpdate = true
  cache.set(kind, texture)
  return texture
}

type FabricMaterial = THREE.Material & {
  isMToonMaterial?: boolean
  isOutline?: boolean
  map?: THREE.Texture | null
  shadeMultiplyTexture?: THREE.Texture | null
}

/** Põe o tecido nas peças sem textura (cor chapada); quem já tem mapa e os contornos ficam como estão. */
export function applyFabricDetail(root: THREE.Object3D): void {
  root.traverse((object) => {
    const mesh = object as THREE.Mesh
    if (!mesh.isMesh) return
    for (const material of (Array.isArray(mesh.material) ? mesh.material : [mesh.material]) as FabricMaterial[]) {
      if (!material.isMToonMaterial || material.isOutline || material.map) continue
      const kind = fabricKindFor(material.name)
      if (!kind) continue
      const texture = fabricTexture(kind)
      material.map = texture
      // A sombra segue o mesmo padrão: sem isto a trama some exatamente nas dobras.
      material.shadeMultiplyTexture = texture
      material.needsUpdate = true
    }
  })
}
