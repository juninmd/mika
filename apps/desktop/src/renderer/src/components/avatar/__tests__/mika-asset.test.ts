import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { inflateSync } from 'node:zlib'
import { describe, expect, it } from 'vitest'

/**
 * Contrato do `mika.vrm` entregue: o redesenho (cabelo preto com pontas azuis, jaqueta nerd-fofa) é feito
 * por `scripts/avatar/restyle-mika.py` direto no glTF. O que este teste protege é o que NÃO pode mudar —
 * esqueleto, spring bones, expressões, malhas — e o que o redesenho prometeu mudar.
 */
interface Mtoon {
  shadeColorFactor?: number[]
  shadeMultiplyTexture?: { index: number }
}
interface Gltf {
  nodes: unknown[]
  meshes: unknown[]
  skins: unknown[]
  accessors: unknown[]
  materials: {
    name: string
    pbrMetallicRoughness: { baseColorTexture?: { index: number }; baseColorFactor?: number[] }
    extensions: { VRMC_materials_mtoon: Mtoon }
  }[]
  textures: { source: number }[]
  images: { bufferView: number }[]
  bufferViews: { byteOffset?: number; byteLength: number }[]
  extensions: {
    VRMC_vrm: {
      specVersion: string
      humanoid: { humanBones: Record<string, unknown> }
      expressions: { preset: Record<string, unknown> }
    }
    VRMC_springBone: { springs: unknown[] }
  }
}

const file = readFileSync(resolve('apps/desktop/src/renderer/public/vrm/mika/mika.vrm'))
const jsonLength = file.readUInt32LE(12)
const gltf = JSON.parse(file.subarray(20, 20 + jsonLength).toString()) as Gltf
const bin = file.subarray(20 + jsonLength + 8)

function imageOf(materialName: string): Buffer {
  const material = gltf.materials.find((m) => m.name === materialName)
  const texture = material?.pbrMetallicRoughness.baseColorTexture?.index
  if (texture === undefined) throw new Error(`${materialName}: sem textura`)
  const view = gltf.bufferViews[gltf.images[gltf.textures[texture].source].bufferView]
  return bin.subarray(view.byteOffset ?? 0, (view.byteOffset ?? 0) + view.byteLength)
}

function idatOf(data: Buffer): Buffer {
  const chunks: Buffer[] = []
  for (let at = 8; at < data.length; ) {
    const length = data.readUInt32BE(at)
    if (data.toString('ascii', at + 4, at + 8) === 'IDAT') chunks.push(data.subarray(at + 8, at + 8 + length))
    at += 12 + length
  }
  return Buffer.concat(chunks)
}

function paeth(left: number, up: number, upLeft: number): number {
  const p = left + up - upLeft
  const [pa, pb, pc] = [Math.abs(p - left), Math.abs(p - up), Math.abs(p - upLeft)]
  if (pa <= pb && pa <= pc) return left
  return pb <= pc ? up : upLeft
}

/** Desfaz os filtros de linha do PNG (0 nenhum, 1 esquerda, 2 cima, 3 média, 4 Paeth). */
function unfilter(raw: Buffer, stride: number, height: number, channels: number): Buffer {
  const pixels = Buffer.alloc(stride * height)
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)]
    for (let i = 0; i < stride; i++) {
      const left = i >= channels ? pixels[y * stride + i - channels] : 0
      const up = y > 0 ? pixels[(y - 1) * stride + i] : 0
      const upLeft = y > 0 && i >= channels ? pixels[(y - 1) * stride + i - channels] : 0
      const predictor = [0, left, up, (left + up) >> 1, paeth(left, up, upLeft)][filter]
      pixels[y * stride + i] = (raw[y * (stride + 1) + 1 + i] + predictor) & 255
    }
  }
  return pixels
}

/** PNG 8 bits RGBA/RGB sem entrelaçamento: o bastante para as texturas que o script grava. */
function decodePng(data: Buffer): {
  width: number
  height: number
  at: (x: number, y: number) => [number, number, number]
} {
  const width = data.readUInt32BE(16)
  const height = data.readUInt32BE(20)
  const channels = data[25] === 6 ? 4 : 3
  const stride = width * channels
  const pixels = unfilter(inflateSync(idatOf(data)), stride, height, channels)
  return {
    width,
    height,
    at: (x, y) => {
      const o = y * stride + x * channels
      return [pixels[o], pixels[o + 1], pixels[o + 2]]
    },
  }
}

/** Média da região (fração do mapa) — um pixel solto cairia num fio de brilho. */
function average(image: ReturnType<typeof decodePng>, y0: number, y1: number): [number, number, number] {
  const sum = [0, 0, 0]
  let count = 0
  for (let y = Math.floor(y0 * image.height); y < Math.floor(y1 * image.height); y += 3) {
    for (let x = 0; x < image.width; x += 5) {
      const [r, g, b] = image.at(x, y)
      sum[0] += r
      sum[1] += g
      sum[2] += b
      count++
    }
  }
  return [sum[0] / count, sum[1] / count, sum[2] / count]
}

describe('mika.vrm — o que o redesenho não pode quebrar', () => {
  it('continua um VRM 1.0 completo: esqueleto, expressões, spring bones, malhas e materiais', () => {
    expect(gltf.extensions.VRMC_vrm.specVersion).toBe('1.0')
    expect(Object.keys(gltf.extensions.VRMC_vrm.humanoid.humanBones)).toHaveLength(54)
    expect(Object.keys(gltf.extensions.VRMC_vrm.expressions.preset)).toHaveLength(18)
    expect(gltf.extensions.VRMC_springBone.springs).toHaveLength(22)
    expect([gltf.nodes.length, gltf.meshes.length, gltf.skins.length, gltf.accessors.length]).toEqual([188, 20, 1, 995])
    expect(gltf.materials).toHaveLength(28)
  })
})

describe('mika.vrm — cabelo preto com pontas azuis', () => {
  for (const name of ['Hair_00_HAIR', 'HairBack_00_HAIR']) {
    it(`${name}: raiz preta, pontas azuis`, () => {
      const image = decodePng(imageOf(name))
      const [rr, rg, rb] = average(image, 0.02, 0.3)
      const [tr, tg, tb] = average(image, 0.9, 1)
      // raiz: escura e sem dominância de cor (preto azulado)
      expect(Math.max(rr, rg, rb)).toBeLessThan(75)
      // ponta: azul claramente dominante e bem mais clara que a raiz
      expect(tb).toBeGreaterThan(190)
      expect(tb).toBeGreaterThan(tr + 60)
      expect(tb + tg + tr).toBeGreaterThan(rr + rg + rb + 300)
    })
  }
})

describe('mika.vrm — jaqueta nerd e feminina', () => {
  it('jaqueta e mangas ganharam estampa (textura própria, base branca para a cor vir do desenho)', () => {
    for (const name of ['Renewal_Jacket', 'Renewal_Sleeves']) {
      const material = gltf.materials.find((m) => m.name === name)
      expect(material?.pbrMetallicRoughness.baseColorFactor).toEqual([1, 1, 1, 1])
      expect(material?.pbrMetallicRoughness.baseColorTexture).toBeDefined()
      expect(material?.extensions.VRMC_materials_mtoon.shadeMultiplyTexture).toEqual(
        material?.pbrMetallicRoughness.baseColorTexture,
      )
    }
  })

  it('a jaqueta é rosa pastel (não mais azul-marinho) e as mangas lilás claro', () => {
    const jacket = average(decodePng(imageOf('Renewal_Jacket')), 0, 1)
    expect(jacket[0]).toBeGreaterThan(200)
    expect(jacket[0]).toBeGreaterThan(jacket[1] + 20)
    const sleeves = average(decodePng(imageOf('Renewal_Sleeves')), 0, 1)
    expect(sleeves[2]).toBeGreaterThan(sleeves[1] + 10)
    expect(Math.min(...sleeves)).toBeGreaterThan(170)
    // Duas texturas decodificadas em JS puro (zlib + filtros PNG): CPU-bound, estourou os 5 s padrão no runner sob carga.
  }, 30_000)
})
