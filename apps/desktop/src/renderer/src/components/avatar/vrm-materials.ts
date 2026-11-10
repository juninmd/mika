/** Acabamento anime aplicado em runtime aos MToon de qualquer VRM VRoid. */
import * as THREE from 'three'

type MToonLike = THREE.Material & {
  isMToonMaterial?: boolean
  isOutline?: boolean
  outlineWidthFactor?: number
  map?: THREE.Texture | null
  shadeMultiplyTexture?: THREE.Texture | null
  normalMap?: THREE.Texture | null
  emissiveMap?: THREE.Texture | null
  shadeColorFactor?: THREE.Color
  shadingShiftFactor?: number
  parametricRimColorFactor?: THREE.Color
  customProgramCacheKey: () => string
}

const EYE = /^Eye(Iris|White|Highlight)/i
const FACE_SKIN = /^Face_\d+_SKIN/i
const SKIN = /_SKIN$/i
/** Sombra no rosto só na borda: cortar bochecha e canto do olho é o que mais "desanimiza" um VRoid. */
export const FACE_SHADING_SHIFT_MIN = 0.5
/** Coral quente: pele com sombra cinza lê como boneco de plástico. */
const WARM_SKIN_HUE = 0.985
const WARM_SKIN_PULL = 0.6
const SKIN_SHADE_MIN_SATURATION = 0.32
/** Quanto o contorno engrossa na silhueta, onde a normal fica de lado para a câmera. */
export const SILHOUETTE_INK_BOOST = 0.6

const OUTLINE_ANCHOR = 'vec3 outlineOffset = outlineWidthFactor * worldNormalLength * objectNormal;'
const OUTLINE_PATCH = `${OUTLINE_ANCHOR}
    outlineOffset *= 1.0 + ( 1.0 - abs( normalize( transformedNormal ).z ) ) * ${SILHOUETTE_INK_BOOST.toFixed(2)};`

export type MaterialRole = 'eye' | 'face' | 'skin' | 'other'

export function materialRole(rawName: string): MaterialRole {
  // O contorno do three-vrm leva o nome do material com " (Outline)" no fim.
  const name = rawName.replace(/ \(Outline\)$/, '')
  if (EYE.test(name)) return 'eye'
  if (FACE_SKIN.test(name)) return 'face'
  if (SKIN.test(name)) return 'skin'
  return 'other'
}

/** Puxa a sombra da pele para o coral mantendo a luminância autoral. */
export function warmSkinShade(color: THREE.Color): void {
  const hsl = { h: 0, s: 0, l: 0 }
  color.getHSL(hsl)
  const delta = ((WARM_SKIN_HUE - hsl.h + 1.5) % 1) - 0.5
  const hue = (hsl.h + delta * WARM_SKIN_PULL + 1) % 1
  color.setHSL(hue, Math.max(hsl.s, SKIN_SHADE_MIN_SATURATION), hsl.l)
}

function tuneRole(material: MToonLike, role: MaterialRole): void {
  if (role === 'eye') {
    // Olho sempre iluminado: íris e brilho com sombra ficam opacos, sem vida.
    material.shadeColorFactor?.setRGB(1, 1, 1)
    material.shadingShiftFactor = 1
    material.parametricRimColorFactor?.setRGB(0, 0, 0)
  } else if (role === 'face') {
    material.shadingShiftFactor = Math.max(material.shadingShiftFactor ?? 0, FACE_SHADING_SHIFT_MIN)
    material.parametricRimColorFactor?.setRGB(0, 0, 0)
    if (material.shadeColorFactor) warmSkinShade(material.shadeColorFactor)
  } else if (role === 'skin' && material.shadeColorFactor) {
    warmSkinShade(material.shadeColorFactor)
  }
}

/** Traço que engrossa na silhueta e afina de frente: leitura de desenho, não de casca inflada. */
function patchOutline(material: MToonLike): void {
  const compile = material.onBeforeCompile
  const cacheKey = material.customProgramCacheKey
  material.onBeforeCompile = (shader, renderer) => {
    compile.call(material, shader, renderer)
    shader.vertexShader = shader.vertexShader.replace(OUTLINE_ANCHOR, OUTLINE_PATCH)
  }
  material.customProgramCacheKey = () => `${cacheKey.call(material)},mika-silhouette-ink`
}

export function tuneAnimeMaterials(root: THREE.Object3D): void {
  const seen = new Set<THREE.Material>()
  root.traverse((object) => {
    const mesh = object as THREE.Mesh
    if (!mesh.isMesh) return
    for (const material of (Array.isArray(mesh.material) ? mesh.material : [mesh.material]) as MToonLike[]) {
      if (seen.has(material) || !material.isMToonMaterial) continue
      seen.add(material)
      const role = materialRole(material.name)
      if (material.isOutline) patchOutline(material)
      else tuneRole(material, role)
      material.needsUpdate = true
    }
  })
}

/**
 * Espessura mínima do traço em pixels CSS. O VRoid autora o contorno em metros (0,4–0,7 mm), que com o
 * corpo inteiro na janela dá ~0,25 px: some, e o desenho de linha que a Live2D tem não existe. Pele leva
 * menos que roupa e cabelo, senão o queixo vira moldura.
 */
const INK_CSS_PX: Record<MaterialRole, number> = { eye: 0, face: 0.8, skin: 0.8, other: 1.15 }
/** Teto em metros: numa janela minúscula o piso em pixels não pode virar contorno de desenho animado. */
const INK_MAX_WIDTH = 0.008
const AUTHORED_INK = 'mikaAuthoredInk'

/** Metros no mundo por pixel CSS, à distância do modelo. */
export function worldPerPixel(distance: number, fovDeg: number, cssHeight: number): number {
  return (2 * Math.max(0, distance) * Math.tan((fovDeg * Math.PI) / 360)) / Math.max(1, cssHeight)
}

export function inkWidth(authored: number, role: MaterialRole, worldPerPx: number): number {
  const floor = Math.min(INK_MAX_WIDTH, INK_CSS_PX[role] * worldPerPx)
  return Math.max(authored, floor)
}

/** Dá ao contorno o piso em pixels de tela; guarda o valor autoral para reaplicar sem compor. */
export function applyOutlineInk(root: THREE.Object3D, worldPerPx: number): void {
  root.traverse((object) => {
    const mesh = object as THREE.Mesh
    if (!mesh.isMesh) return
    for (const material of (Array.isArray(mesh.material) ? mesh.material : [mesh.material]) as MToonLike[]) {
      if (!material.isOutline || material.outlineWidthFactor === undefined) continue
      const authored = (material.userData[AUTHORED_INK] as number | undefined) ?? material.outlineWidthFactor
      material.userData[AUTHORED_INK] = authored
      material.outlineWidthFactor = inkWidth(authored, materialRole(material.name), worldPerPx)
    }
  })
}

const TEXTURE_SLOTS = ['map', 'shadeMultiplyTexture', 'normalMap', 'emissiveMap'] as const

/**
 * Filtragem anisotrópica e trilinear nos mapas com mipmap: de frente ou de lado, pele, rosto e cabelo
 * ficam nítidos em vez de borrar e cintilar quando a avatar se inclina.
 */
export function sharpenTextures(root: THREE.Object3D, anisotropy: number): void {
  root.traverse((object) => {
    const mesh = object as THREE.Mesh
    if (!mesh.isMesh) return
    for (const material of (Array.isArray(mesh.material) ? mesh.material : [mesh.material]) as MToonLike[]) {
      for (const slot of TEXTURE_SLOTS) {
        const texture = material[slot]
        if (!texture?.generateMipmaps) continue
        texture.anisotropy = Math.max(texture.anisotropy, anisotropy)
        texture.minFilter = THREE.LinearMipmapLinearFilter
        texture.needsUpdate = true
      }
    }
  })
}
