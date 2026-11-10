import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { AVATAR_MODELS, LIVE2D_MODELS } from '../../renderer/src/components/live2d/model-path'
import { DEFAULT_MODEL_PATH } from '../avatar-model'

/** O par schema↔renderer do modelo default vive em Rust (`config/schemas/character.rs`). */
describe('avatar padrão', () => {
  it('o modelo padrão vai dentro do instalador', () => {
    const model = AVATAR_MODELS.find((option) => option.path === DEFAULT_MODEL_PATH)

    expect(model?.label).toBe('Mika Aurora (VRM)')
    expect(model?.bundled).toBe(true)
  })

  it('nenhuma amostra Live2D não distribuída pode ser o padrão', () => {
    const unbundled = LIVE2D_MODELS.filter((option) => !option.bundled).map((option) => option.path)

    expect(unbundled).not.toContain(DEFAULT_MODEL_PATH)
    expect(unbundled.length).toBeGreaterThan(0)
  })
})

const PUBLIC_DIR = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'renderer', 'public')

/** Percorre os chunks de um `.glb`/`.vrm` e devolve o que o cabeçalho diz sobre eles. */
function readGlbChunks(bytes: Buffer): { magic: string; declaredLength: number; chunks: string[]; endsAt: number } {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const magic = bytes.subarray(0, 4).toString('ascii')
  const declaredLength = view.getUint32(8, true)

  const chunks: string[] = []
  let offset = 12
  while (offset + 8 <= bytes.byteLength) {
    const chunkLength = view.getUint32(offset, true)
    chunks.push(
      bytes
        .subarray(offset + 4, offset + 8)
        .toString('ascii')
        .replace(/[^A-Za-z]/g, ''),
    )
    offset += 8 + chunkLength
  }
  return { magic, declaredLength, chunks, endsAt: offset }
}

/** `.gitattributes` faltava `.vrm` como binário; o commit corrompeu bytes sem mudar o tamanho do arquivo, invisível a lint/typecheck/diff. */
describe('modelo VRM embarcado', () => {
  const bytes = readFileSync(join(PUBLIC_DIR, DEFAULT_MODEL_PATH))

  it('é um glTF binário íntegro, do cabeçalho ao último chunk', () => {
    const { magic, declaredLength, chunks, endsAt } = readGlbChunks(bytes)

    expect(magic).toBe('glTF')
    expect(chunks).toEqual(['JSON', 'BIN'])
    // Passar do fim é exatamente o que estourava o DataView; parar antes deixaria textura de fora.
    expect(declaredLength).toBe(bytes.byteLength)
    expect(endsAt).toBe(bytes.byteLength)
  })

  it('carrega a Aurora aprovada com rig e fala dentro do orçamento de malha', () => {
    const jsonLength = bytes.readUInt32LE(12)
    const gltf = JSON.parse(bytes.subarray(20, 20 + jsonLength).toString('utf8'))
    const vrm = gltf.extensions.VRMC_vrm

    expect(vrm.meta.name).toBe('Mika Aurora - Blender Arcade')
    expect(vrm.humanoid.humanBones.head).toBeDefined()
    for (const expression of ['blink', 'aa', 'ih', 'ou', 'ee', 'oh']) {
      expect(vrm.expressions.preset[expression]?.morphTargetBinds.length, expression).toBeGreaterThan(0)
    }
    const triangles = gltf.meshes.reduce(
      (total: number, mesh: { primitives: { indices: number }[] }) =>
        total + mesh.primitives.reduce((count, primitive) => count + gltf.accessors[primitive.indices].count / 3, 0),
      0,
    )
    expect(triangles).toBeLessThanOrEqual(140_000)
  })
})
