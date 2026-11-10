import { MToonMaterial } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { applyFabricDetail, type FabricKind, fabricKindFor, fabricPixels } from '../vrm-fabric'

const KINDS: FabricKind[] = ['knit', 'twill', 'rib', 'pleat']

const mtoon = (name: string, extra: Record<string, unknown> = {}) => {
  const material = new MToonMaterial(extra)
  material.name = name
  return material
}
const meshWith = (...materials: THREE.Material[]) => new THREE.Mesh(new THREE.BufferGeometry(), materials)

describe('tecido procedural', () => {
  it.each(KINDS)('%s: cinza neutro, opaco e claro o bastante para não escurecer a cor autoral', (kind) => {
    const pixels = fabricPixels(kind)
    let min = 255
    let sum = 0
    for (let i = 0; i < pixels.length; i += 4) {
      expect(pixels[i]).toBe(pixels[i + 1])
      expect(pixels[i]).toBe(pixels[i + 2])
      expect(pixels[i + 3]).toBe(255)
      min = Math.min(min, pixels[i])
      sum += pixels[i]
    }
    expect(min).toBeGreaterThanOrEqual(0.78 * 255)
    // média acima de 0,9: a peça não pode ficar visivelmente mais escura que a cor do autor
    expect(sum / (pixels.length / 4)).toBeGreaterThan(0.9 * 255)
  })

  it.each(KINDS)('%s: padrão com relevo (não é uma cor lisa) e determinístico', (kind) => {
    const a = fabricPixels(kind)
    const b = fabricPixels(kind)
    expect(Array.from(a)).toEqual(Array.from(b))
    expect(new Set(Array.from(a).filter((_, i) => i % 4 === 0)).size).toBeGreaterThan(8)
  })

  it.each(KINDS)('%s: repete sem emenda (a borda de um lado continua na do outro)', (kind) => {
    const pixels = fabricPixels(kind)
    const size = Math.sqrt(pixels.length / 4)
    const at = (col: number, row: number) => pixels[(row * size + col) * 4]
    for (let row = 0; row < size; row += 7) {
      // o pixel "size" seria o 0 do próximo mosaico: o salto entre vizinhos da emenda é um passo normal
      const step = Math.abs(at(size - 1, row) - at(0, row))
      const inner = Math.abs(at(1, row) - at(0, row))
      expect(step).toBeLessThanOrEqual(Math.max(inner, 8) * 2 + 6)
    }
  })

  it('escolhe o tecido pelo nome do material do VRoid/Blender', () => {
    expect(fabricKindFor('Renewal_Cuff_Rib')).toBe('rib')
    expect(fabricKindFor('Renewal_arcade_Skirt')).toBe('pleat')
    expect(fabricKindFor('Renewal_Sleeves')).toBe('knit')
    expect(fabricKindFor('Renewal_Crewneck')).toBe('knit')
    expect(fabricKindFor('Renewal_1_Sock')).toBe('knit')
    expect(fabricKindFor('Renewal_Jacket')).toBe('twill')
    expect(fabricKindFor('Renewal_Inner')).toBe('twill')
    expect(fabricKindFor('Renewal_1_SockStripe')).toBeUndefined()
    expect(fabricKindFor('Body_00_SKIN')).toBeUndefined()
    expect(fabricKindFor('Haru_Blazer')).toBeUndefined()
  })

  it('só toca em peça sem textura, nunca no contorno nem no que já tem mapa', () => {
    const flat = mtoon('Renewal_Sleeves')
    const flat2 = mtoon('Renewal_Jacket')
    const textured = mtoon('Renewal_Inner')
    const own = new THREE.Texture()
    textured.map = own
    const outline = mtoon('Renewal_Sleeves (Outline)')
    outline.isOutline = true
    const skin = mtoon('Body_00_SKIN')
    applyFabricDetail(meshWith(flat, flat2, textured, outline, skin))
    expect(flat.map).toBeInstanceOf(THREE.DataTexture)
    expect(flat.shadeMultiplyTexture).toBe(flat.map)
    expect(flat2.map).toBeInstanceOf(THREE.DataTexture)
    expect(textured.map).toBe(own)
    expect(outline.map).toBeNull()
    expect(skin.map).toBeNull()
  })

  it('peças do mesmo tecido dividem a textura (uma por tipo, não uma por peça)', () => {
    const a = mtoon('Renewal_Sleeves')
    const b = mtoon('Renewal_Crewneck')
    applyFabricDetail(meshWith(a, b))
    expect(a.map).toBe(b.map)
  })
})
