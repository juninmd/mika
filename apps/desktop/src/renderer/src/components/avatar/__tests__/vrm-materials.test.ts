import { MToonMaterial } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import {
  applyOutlineInk,
  FACE_SHADING_SHIFT_MIN,
  inkWidth,
  materialRole,
  sharpenTextures,
  tuneAnimeMaterials,
  warmSkinShade,
  worldPerPixel,
} from '../vrm-materials'

const meshWith = (...materials: THREE.Material[]) => new THREE.Mesh(new THREE.BufferGeometry(), materials)

const mtoon = (name: string, extra: Record<string, unknown> = {}) => {
  const material = new MToonMaterial({ shadingShiftFactor: -0.1, ...extra })
  material.name = name
  return material
}

describe('acabamento anime dos materiais VRoid', () => {
  it('classifica pelos nomes que o VRoid exporta', () => {
    expect(materialRole('EyeIris_00_EYE')).toBe('eye')
    expect(materialRole('EyeHighlight_00_EYE')).toBe('eye')
    expect(materialRole('Face_00_SKIN')).toBe('face')
    expect(materialRole('Body_00_SKIN')).toBe('skin')
    expect(materialRole('Haru_Blazer')).toBe('other')
  })

  /** Íris com sombra fica opaca: o olhar é a primeira coisa que perde a vida num VRoid. */
  it('olho nunca recebe sombra e o rosto só na borda', () => {
    const iris = mtoon('EyeIris_00_EYE')
    const face = mtoon('Face_00_SKIN', { parametricRimColorFactor: new THREE.Color(0.2, 0.2, 0.2) })
    const cloth = mtoon('Haru_Blazer')
    tuneAnimeMaterials(meshWith(iris, face, cloth))
    expect(iris.shadingShiftFactor).toBe(1)
    expect(iris.shadeColorFactor.getHex()).toBe(0xffffff)
    expect(face.shadingShiftFactor).toBe(FACE_SHADING_SHIFT_MIN)
    expect(face.parametricRimColorFactor.getHex()).toBe(0)
    expect(cloth.shadingShiftFactor).toBe(-0.1)
  })

  it('não diminui um shift de rosto que o autor já abriu mais', () => {
    const face = mtoon('Face_00_SKIN', { shadingShiftFactor: 0.8 })
    tuneAnimeMaterials(meshWith(face))
    expect(face.shadingShiftFactor).toBe(0.8)
  })

  it('sombra da pele esquenta sem mudar a luminância autoral', () => {
    const shade = new THREE.Color().setHSL(0.6, 0.1, 0.4)
    warmSkinShade(shade)
    const hsl = { h: 0, s: 0, l: 0 }
    shade.getHSL(hsl)
    expect(hsl.l).toBeCloseTo(0.4, 2)
    expect(hsl.s).toBeGreaterThanOrEqual(0.31)
    expect(Math.min(Math.abs(hsl.h - 0.985), 1 - Math.abs(hsl.h - 0.985))).toBeLessThan(0.25)
  })

  /** A âncora é texto do shader do three-vrm: uma atualização que a mude desliga o traço em silêncio. */
  it('o contorno engrossa na silhueta no shader real do MToon', () => {
    const outline = mtoon('Hair_00_HAIR (Outline)', { outlineWidthMode: 'worldCoordinates', outlineWidthFactor: 0.001 })
    outline.isOutline = true
    const key = outline.customProgramCacheKey()
    tuneAnimeMaterials(meshWith(outline))
    const shader = { vertexShader: outline.vertexShader, fragmentShader: outline.fragmentShader, uniforms: {} }
    outline.onBeforeCompile(shader as never, {} as never)
    expect(shader.vertexShader).toContain('abs( normalize( transformedNormal ).z )')
    expect(outline.customProgramCacheKey()).toBe(`${key},mika-silhouette-ink`)
  })
})

/**
 * O traço do VRoid vem em metros (0,4–0,7 mm): com o corpo inteiro em ~800 px isso é ~0,25 px — some,
 * e a avatar perde o desenho de linha que a Live2D tem. A espessura passa a ter piso em pixels de tela.
 */
describe('traço de tinta em pixels de tela', () => {
  it('worldPerPixel: metros por pixel CSS à distância do modelo', () => {
    // fov 28°, 3.4 m, 800 px: 2·3.4·tan(14°)/800
    expect(worldPerPixel(3.4, 28, 800)).toBeCloseTo((2 * 3.4 * Math.tan((14 * Math.PI) / 180)) / 800, 8)
    expect(worldPerPixel(3.4, 28, 0)).toBeGreaterThan(0)
  })

  it('o piso em pixels vence o valor autoral minúsculo, e o autoral vence se for maior', () => {
    const wpp = 0.002
    expect(inkWidth(0.0005, 'other', wpp)).toBeGreaterThan(0.0005)
    expect(inkWidth(0.02, 'other', wpp)).toBe(0.02)
  })

  it('pele leva traço mais fino que roupa e cabelo', () => {
    expect(inkWidth(0, 'skin', 0.002)).toBeLessThan(inkWidth(0, 'other', 0.002))
  })

  it('há um teto: janela minúscula não vira contorno de desenho animado', () => {
    expect(inkWidth(0.0005, 'other', 1)).toBeLessThanOrEqual(0.008)
  })

  it('aplica só nos contornos, por papel, e reaplicar não compõe', () => {
    const outline = mtoon('Hair_00_HAIR (Outline)', {
      outlineWidthMode: 'worldCoordinates',
      outlineWidthFactor: 0.0007,
    })
    outline.isOutline = true
    const skinOutline = mtoon('Face_00_SKIN (Outline)', {
      outlineWidthMode: 'worldCoordinates',
      outlineWidthFactor: 0.0004,
    })
    skinOutline.isOutline = true
    const body = mtoon('Hair_00_HAIR', { outlineWidthFactor: 0.0007 })
    const root = meshWith(outline, skinOutline, body)
    applyOutlineInk(root, 0.002)
    const first = outline.outlineWidthFactor
    expect(first).toBeGreaterThan(0.0007)
    expect(skinOutline.outlineWidthFactor).toBeGreaterThan(0.0004)
    expect(skinOutline.outlineWidthFactor).toBeLessThan(first)
    expect(body.outlineWidthFactor).toBe(0.0007)
    applyOutlineInk(root, 0.002)
    expect(outline.outlineWidthFactor).toBe(first)
    // janela maior (menos metros por pixel): o traço recua em direção ao autoral, não fica preso no maior
    applyOutlineInk(root, 0.0005)
    expect(outline.outlineWidthFactor).toBeLessThan(first)
  })
})

describe('nitidez das texturas', () => {
  it('anisotropia e trilinear nos mapas de cor, sem tocar nos que não têm mipmap', () => {
    const color = new THREE.Texture()
    color.minFilter = THREE.LinearMipmapNearestFilter
    const flat = new THREE.Texture()
    flat.generateMipmaps = false
    flat.minFilter = THREE.LinearFilter
    const material = mtoon('Body_00_SKIN') as unknown as {
      map: THREE.Texture | null
      shadeMultiplyTexture: THREE.Texture | null
    }
    material.map = color
    material.shadeMultiplyTexture = flat
    sharpenTextures(meshWith(material as unknown as THREE.Material), 8)
    expect(color.anisotropy).toBe(8)
    expect(color.minFilter).toBe(THREE.LinearMipmapLinearFilter)
    expect(flat.anisotropy).toBe(1)
    expect(flat.minFilter).toBe(THREE.LinearFilter)
  })
})
