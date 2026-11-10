import { MToonMaterial } from '@pixiv/three-vrm'
import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { applyHairShine, isHairMaterial, SHINE_STRENGTH, shinePixels } from '../vrm-shine'

const mtoon = (name: string) => {
  const material = new MToonMaterial()
  material.name = name
  return material
}
const meshWith = (...materials: THREE.Material[]) => new THREE.Mesh(new THREE.BufferGeometry(), materials)

describe('anel de anjo do cabelo (matcap)', () => {
  const size = 128
  const pixels = shinePixels()
  const at = (col: number, row: number) => pixels[(row * size + col) * 4]

  it('é uma faixa horizontal: o centro da faixa brilha, o resto da esfera fica preto', () => {
    const peakRow = Math.round(0.3 * size)
    expect(at(size / 2, peakRow)).toBeGreaterThan(150)
    expect(at(size / 2, 2)).toBe(0)
    expect(at(size / 2, size - 3)).toBe(0)
  })

  it('some nas laterais, onde a normal já vira de lado, e é simétrica', () => {
    const row = Math.round(0.3 * size)
    expect(at(2, row)).toBe(0)
    expect(at(size - 3, row)).toBe(0)
    expect(at(40, row)).toBe(at(size - 1 - 40, row))
  })

  it('opaca e sem estourar: nunca passa do máximo e o tom é lavanda, não branco puro', () => {
    for (let i = 0; i < pixels.length; i += 4) expect(pixels[i + 3]).toBe(255)
    const center = (Math.round(0.3 * size) * size + size / 2) * 4
    expect(pixels[center]).toBeLessThan(255)
    expect(pixels[center + 2]).toBeGreaterThan(pixels[center])
  })

  it('só o cabelo, nunca o contorno nem quem já tem matcap', () => {
    expect(isHairMaterial('Hair_00_HAIR')).toBe(true)
    expect(isHairMaterial('HairBack_00_HAIR')).toBe(true)
    expect(isHairMaterial('Body_00_SKIN')).toBe(false)
    expect(isHairMaterial('Hair_00_HAIR (Outline)')).toBe(false)

    const hair = mtoon('Hair_00_HAIR')
    const back = mtoon('HairBack_00_HAIR')
    const skin = mtoon('Body_00_SKIN')
    const outline = mtoon('Hair_00_HAIR (Outline)')
    outline.isOutline = true
    const own = new THREE.Texture()
    const authored = mtoon('Hair_00_HAIR')
    authored.matcapTexture = own
    applyHairShine(meshWith(hair, back, skin, outline, authored))
    expect(hair.matcapTexture).toBeInstanceOf(THREE.DataTexture)
    expect(back.matcapTexture).toBe(hair.matcapTexture)
    expect(hair.matcapFactor.r).toBeCloseTo(SHINE_STRENGTH, 5)
    expect(skin.matcapTexture).toBeNull()
    expect(outline.matcapTexture).toBeNull()
    expect(authored.matcapTexture).toBe(own)
  })
})
