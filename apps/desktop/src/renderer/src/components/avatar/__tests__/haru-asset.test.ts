import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { HARU_OUTFITS } from '../vrm-look'

interface Accessor {
  bufferView: number
  byteOffset?: number
  count: number
  type: string
  componentType: number
}
interface Primitive {
  attributes: Record<string, number>
  material: number
  indices: number
}
interface Asset {
  buffers: { byteLength: number }[]
  bufferViews: { byteOffset: number; byteLength: number }[]
  accessors: Accessor[]
  skins: { joints: number[]; inverseBindMatrices: number }[]
  meshes: { name: string; primitives: Primitive[] }[]
  nodes: { children?: number[]; name: string; mesh?: number; skin?: number }[]
  images: { name?: string; bufferView: number }[]
  materials: { name: string; extensions?: { VRMC_materials_mtoon?: { outlineWidthFactor?: number } } }[]
  extensions: {
    VRMC_vrm: {
      meta: { version: string; authors: string[]; modification: string }
      expressions: { preset: Record<string, unknown> }
    }
    VRMC_springBone: {
      springs: { name: string; joints: { node: number }[]; colliderGroups?: number[] }[]
      colliderGroups: { colliders: number[] }[]
      colliders: { node: number }[]
    }
  }
}

const bytes = readFileSync(resolve('apps/desktop/src/renderer/public/vrm/mika/haru.vrm'))
const jsonEnd = 20 + bytes.readUInt32LE(12)
const asset: Asset = JSON.parse(bytes.subarray(20, jsonEnd).toString())
const binary = bytes.subarray(jsonEnd + 8)

function values(index: number): number[] {
  const accessor = asset.accessors[index]
  const offset = asset.bufferViews[accessor.bufferView].byteOffset + (accessor.byteOffset ?? 0)
  const components: Record<string, number> = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 }
  return Array.from({ length: accessor.count * components[accessor.type] }, (_, i) =>
    accessor.componentType === 5126 ? binary.readFloatLE(offset + i * 4) : binary.readUInt16LE(offset + i * 2),
  )
}

function expectPrimitiveWeightsAndCoordsValid(node: Asset['nodes'][number]): void {
  const skin = asset.skins[node.skin ?? 0]
  for (const primitive of asset.meshes[node.mesh ?? 0].primitives) {
    const weights = values(primitive.attributes.WEIGHTS_0)
    const joints = values(primitive.attributes.JOINTS_0)
    expect(
      joints.every((joint) => joint >= 0 && joint < skin.joints.length),
      node.name,
    ).toBe(true)
    expect(
      weights.every((weight) => weight >= 0 && weight <= 1),
      node.name,
    ).toBe(true)
    for (let i = 0; i < weights.length; i += 4) {
      const sum = weights[i] + weights[i + 1] + weights[i + 2] + weights[i + 3]
      if (Math.abs(sum - 1) > 0.00001) throw new Error(`${node.name}: soma de pesos ${sum}`)
    }
    for (const channel of ['POSITION', 'NORMAL', 'TEXCOORD_0']) {
      expect(values(primitive.attributes[channel]).every(Number.isFinite), `${node.name}.${channel}`).toBe(true)
    }
  }
}

function expectMeshIndicesWithinVertexCount(mesh: Asset['meshes'][number]): void {
  for (const primitive of mesh.primitives) {
    const accessor = asset.accessors[primitive.indices]
    const view = asset.bufferViews[accessor.bufferView]
    const offset = view.byteOffset + (accessor.byteOffset ?? 0)
    const count = asset.accessors[primitive.attributes.POSITION].count
    for (let i = 0; i < accessor.count; i++) {
      const index =
        accessor.componentType === 5125 ? binary.readUInt32LE(offset + i * 4) : binary.readUInt16LE(offset + i * 2)
      if (index >= count) throw new Error(`${mesh.name}: índice ${index} excede ${count} vértices`)
    }
  }
}

function expectSpringColliderGroupsValid(spring: Asset['extensions']['VRMC_springBone']['springs'][number]): void {
  expect(spring.colliderGroups?.length).toBeGreaterThan(0)
  for (const groupIndex of spring.colliderGroups ?? []) {
    const group = asset.extensions.VRMC_springBone.colliderGroups[groupIndex]
    expect(group).toBeDefined()
    for (const colliderIndex of group.colliders) {
      const collider = asset.extensions.VRMC_springBone.colliders[colliderIndex]
      expect(collider).toBeDefined()
      expect(asset.nodes[collider.node]).toBeDefined()
    }
  }
}

function expectSpringJointChainValid(
  spring: Asset['extensions']['VRMC_springBone']['springs'][number],
  seen: Set<number>,
): void {
  expect(spring.joints.length).toBeGreaterThanOrEqual(3)
  for (const [i, joint] of spring.joints.entries()) {
    expect(seen.has(joint.node)).toBe(false)
    seen.add(joint.node)
    expect(asset.skins[0].joints).toContain(joint.node)
    if (i > 0) expect(asset.nodes[spring.joints[i - 1].node].children).toContain(joint.node)
  }
}

describe('artefato Haru VRM distribuído', () => {
  // Quality over size (owner's call, 2026-09-25); the ceiling only catches orphaned bytes piling up.
  it('é um GLB completo com texturas embutidas e sem bytes órfãos acumulados', () => {
    expect(bytes.readUInt32LE(0)).toBe(0x46546c67)
    expect(bytes.readUInt32LE(8)).toBe(bytes.length)
    expect(asset.buffers[0].byteLength).toBe(binary.length)
    expect(bytes.length).toBeLessThan(64 * 1024 * 1024)
    for (const view of asset.bufferViews) expect(view.byteOffset + view.byteLength).toBeLessThanOrEqual(binary.length)
    expect(asset.images.map((image) => image.name)).toEqual(
      expect.arrayContaining([
        'Haru_EyeIris_00',
        'Haru_Hair_00',
        'Haru_Body_00',
        'Haru_WovenCloth',
        'Haru_DiagonalSilk',
      ]),
    )
  })

  it('mantém autoria, permissão de redistribuição e morphs faciais do modelo base', () => {
    const vrm = asset.extensions.VRMC_vrm
    expect(vrm.meta.version).toBe('3.0.0')
    expect(vrm.meta.authors).toContain('pixiv Inc.')
    expect(vrm.meta.modification).toBe('allowModificationRedistribution')
    for (const expression of ['happy', 'sad', 'blink', 'blinkLeft', 'aa', 'ih', 'ou', 'ee', 'oh']) {
      expect(vrm.expressions.preset).toHaveProperty(expression)
    }
  })

  it('toda peça gerada tem pesos normalizados, ossos existentes e coordenadas finitas', () => {
    for (const skin of asset.skins) expect(asset.accessors[skin.inverseBindMatrices].count).toBe(skin.joints.length)
    for (const node of asset.nodes.filter((entry) => /^(Haru|Outfit)_/.test(entry.name) && entry.mesh !== undefined)) {
      expectPrimitiveWeightsAndCoordsValid(node)
    }
  })

  // Each outfit look reveals meshes by name prefix: a piece named off-pattern would never show up.
  it('traz saia e peças próprias de cada traje, ocultas pelo nome', () => {
    for (const look of HARU_OUTFITS) {
      const pieces = asset.nodes.filter((node) => node.mesh !== undefined && look.show?.test(node.name))
      expect(
        pieces.map((node) => node.name),
        look.id,
      ).toContain(`Outfit_${look.name}_Skirt`)
      expect(pieces.length, look.id).toBeGreaterThanOrEqual(4)
    }
  })

  it('corpo e cabelo usam malha densa com índices que não excedem os vértices', () => {
    for (const mesh of asset.meshes) expectMeshIndicesWithinVertexCount(mesh)
    const body = asset.meshes.find((mesh) => mesh.name === 'Body')?.primitives[0]
    expect(body).toBeDefined()
    if (body) expect(asset.accessors[body.indices].count / 3).toBeGreaterThan(90_000)
  })

  it('toda peça visível é MToon com contorno: PBR sem traço foi o que deixou a Haru com cara de boneco N64', () => {
    const used = new Set(asset.meshes.flatMap((mesh) => mesh.primitives.map((primitive) => primitive.material)))
    for (const index of used) {
      const material = asset.materials[index]
      expect(material.extensions?.VRMC_materials_mtoon, material.name).toBeDefined()
      if (!/EYE|FACE$/.test(material.name)) {
        expect(material.extensions?.VRMC_materials_mtoon?.outlineWidthFactor, material.name).toBeGreaterThan(0)
      }
    }
  })

  it('cacho, lenço e saia têm cadeias físicas com colisores válidos', () => {
    const springs = asset.extensions.VRMC_springBone.springs.filter((spring) => spring.name.startsWith('Haru_'))
    expect(springs).toHaveLength(11)
    expect(springs.filter((spring) => spring.name.startsWith('Haru_Skirt'))).toHaveLength(8)
    const seen = new Set<number>()
    for (const spring of springs) {
      expectSpringColliderGroupsValid(spring)
      expectSpringJointChainValid(spring, seen)
    }
  })
})
