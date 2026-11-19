import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { type VRM, VRMLoaderPlugin } from '@pixiv/three-vrm'
import { createVRMAnimationClip, type VRMAnimation, VRMAnimationLoaderPlugin } from '@pixiv/three-vrm-animation'
import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { describe, expect, it } from 'vitest'

/**
 * Os colisores do cabelo vieram do corpo VRoid; a jaqueta, a camisa e as mangas ficam 1 a 4 cm por fora
 * deles. Num gesto que inclina o tronco, as mechas compridas entravam na roupa ("cabelo atravessando o
 * corpo"). Aqui a física roda de verdade sobre um clipe capturado e conta mechas que cruzam a roupa.
 */
const PUBLIC = 'apps/desktop/src/renderer/public/vrm'
const GARMENT = /Jacket|Sleeves|Cuff|Inner|Crewneck|Skirt|Hem/

/** Textures need a DOM to decode; the physics doesn't use them. */
function withoutTextures(file: Buffer): ArrayBuffer {
  const length = file.readUInt32LE(12)
  const gltf = JSON.parse(file.subarray(20, 20 + length).toString())
  const strip = (node: unknown): void => {
    if (!node || typeof node !== 'object') return
    for (const [key, value] of Object.entries(node)) {
      if (/texture$/i.test(key) && typeof value === 'object') delete (node as Record<string, unknown>)[key]
      else strip(value)
    }
  }
  strip(gltf.materials)
  for (const key of ['textures', 'images', 'samplers']) delete gltf[key]
  delete gltf.extensions?.VRMC_vrm?.meta?.thumbnailImage
  let json = Buffer.from(JSON.stringify(gltf))
  json = Buffer.concat([json, Buffer.alloc((-json.length % 4) & 3, 0x20)])
  const rest = file.subarray(20 + length)
  const header = Buffer.alloc(20)
  header.write('glTF', 0)
  header.writeUInt32LE(2, 4)
  header.writeUInt32LE(20 + json.length + rest.length, 8)
  header.writeUInt32LE(json.length, 12)
  header.write('JSON', 16)
  const out = Buffer.concat([header, json, rest])
  return out.buffer.slice(out.byteOffset, out.byteOffset + out.byteLength)
}

async function load(path: string): Promise<{ vrm?: VRM; vrmAnimations?: VRMAnimation[] }> {
  const loader = new GLTFLoader()
  loader.register((parser) => new VRMLoaderPlugin(parser))
  loader.register((parser) => new VRMAnimationLoaderPlugin(parser))
  const file = readFileSync(resolve(PUBLIC, path))
  const data = path.endsWith('.vrm')
    ? withoutTextures(file)
    : file.buffer.slice(file.byteOffset, file.byteOffset + file.byteLength)
  const gltf = await loader.parseAsync(data, '')
  return gltf.userData
}

function skinnedPositions(mesh: THREE.SkinnedMesh): Float32Array {
  const position = mesh.geometry.attributes.position
  const out = new Float32Array(position.count * 3)
  const v = new THREE.Vector3()
  for (let i = 0; i < position.count; i++) {
    mesh
      .applyBoneTransform(i, v.fromBufferAttribute(position, i))
      .applyMatrix4(mesh.matrixWorld)
      .toArray(out, i * 3)
  }
  return out
}

/** Cheap reject: the triangle's bounding box misses the segment's. */
function outside(p: Float32Array, i: number, j: number, k: number, lo: THREE.Vector3, hi: THREE.Vector3): boolean {
  for (let axis = 0; axis < 3; axis++) {
    const [x, y, z] = [p[i + axis], p[j + axis], p[k + axis]]
    if (Math.max(x, y, z) < lo.getComponent(axis) || Math.min(x, y, z) > hi.getComponent(axis)) return true
  }
  return false
}

/** Hair chain segments (joint → child) that pass through a garment triangle. */
function crossings(vrm: VRM, garments: THREE.SkinnedMesh[]): number {
  vrm.scene.updateMatrixWorld(true)
  const segments: THREE.Vector3[][] = []
  for (const joint of vrm.springBoneManager?.joints ?? []) {
    if (!joint.child || !/Hair/.test(joint.bone.name)) continue
    segments.push([joint.bone.getWorldPosition(new THREE.Vector3()), joint.child.getWorldPosition(new THREE.Vector3())])
  }
  const surfaces = garments.map((mesh) => ({ p: skinnedPositions(mesh), index: mesh.geometry.index?.array ?? [] }))
  const ray = new THREE.Ray()
  const [a, b, c, hit] = [new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3()]
  let count = 0
  for (const [from, to] of segments) {
    const lo = from.clone().min(to).subScalar(0.01)
    const hi = from.clone().max(to).addScalar(0.01)
    const length = from.distanceTo(to)
    ray.set(from, to.clone().sub(from).normalize())
    const crosses = surfaces.some(({ p, index }) => {
      for (let t = 0; t < index.length; t += 3) {
        const [i, j, k] = [index[t] * 3, index[t + 1] * 3, index[t + 2] * 3]
        if (outside(p, i, j, k, lo, hi)) continue
        a.fromArray(p, i)
        b.fromArray(p, j)
        c.fromArray(p, k)
        if (ray.intersectTriangle(a, b, c, false, hit) && hit.distanceTo(from) <= length) return true
      }
      return false
    })
    if (crosses) count++
  }
  return count
}

async function crossingsDuring(clipName: string): Promise<number> {
  const { vrm } = await load('mika/mika.vrm')
  const { vrmAnimations } = await load(`animations/${clipName}.vrma`)
  if (!vrm || !vrmAnimations?.[0]) throw new Error('asset sem VRM')
  const garments: THREE.SkinnedMesh[] = []
  vrm.scene.traverse((node) => {
    const mesh = node as THREE.SkinnedMesh
    const material = mesh.isSkinnedMesh ? ([mesh.material].flat()[0] as THREE.Material).name : ''
    if (GARMENT.test(material)) garments.push(mesh)
  })
  const mixer = new THREE.AnimationMixer(vrm.scene)
  mixer.clipAction(createVRMAnimationClip(vrmAnimations[0], vrm)).play()
  let total = 0
  for (let frame = 1; frame <= 240; frame++) {
    mixer.update(1 / 60)
    vrm.update(1 / 60)
    if (frame > 30 && frame % 12 === 0) total += crossings(vrm, garments)
  }
  return total
}

describe('cabelo da Mika contra a roupa', () => {
  // Antes do ajuste: 66 cruzamentos em Blush e 51 em Thinking (até 8 mechas na roupa no mesmo quadro).
  it.each(['Blush', 'Thinking'])('mechas não atravessam a roupa durante %s', { timeout: 20_000 }, async (clip) => {
    expect(await crossingsDuring(clip)).toBeLessThanOrEqual(6)
  })
})
