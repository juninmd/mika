/** Recria a variante Haru sem alterar o VRM base. node scripts/generators/generate-haru-vrm.mjs */

import { readFileSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { isLeg, sculptLeg } from './haru-anatomy.mjs'
import { compactGltf } from './haru-compact.mjs'
import { animeFacePng, animeMouthPng } from './haru-face.mjs'
import { foldColor } from './haru-folds.mjs'
import {
  curvedPanel,
  fittedSkirt,
  garmentBack,
  garmentFront,
  scarfTail,
  scarfWrap,
  skirtPoint,
  tailoredSleeve,
  tailoredTorso,
  torsoSection,
} from './haru-garments.mjs'
import { drillCurl, haruStylePoint } from './haru-hair.mjs'
import { belowHeight, cuffRing, legRadius, legShell } from './haru-legwear.mjs'
import {
  angelRingColor,
  clothColor,
  HAIR_STOPS,
  hairStrokes,
  IRIS_STOPS,
  inkOutline,
  PALETTE,
  recolorPng,
  scarfColor,
  texturePng,
  toonMaterial,
} from './haru-materials.mjs'
import { addOutfits } from './haru-outfits.mjs'
import { recodePng } from './haru-png.mjs'
import { refineAnimeProportions } from './haru-proportions.mjs'
import { subdividePrimitive } from './haru-subdivision.mjs'
import { warpInPlace } from './haru-warp.mjs'

const require = createRequire(new URL('../../apps/desktop/package.json', import.meta.url))
const T = require('three')
const folder = new URL('../../apps/desktop/src/renderer/public/vrm/mika/', import.meta.url)
// Keep Haru regeneration pinned to the original licensed base. The shipped
// mika.vrm may be the approved Aurora model and must never become generator input.
const source = readFileSync(new URL('assets/mika-base.vrm', import.meta.url))
const jsonLength = source.readUInt32LE(12)
const gltf = JSON.parse(source.subarray(20, 20 + jsonLength).toString())
let binary = Buffer.from(source.subarray(28 + jsonLength))
const bones = gltf.extensions.VRMC_vrm.humanoid.humanBones
const skin = gltf.skins[0]

function data(index) {
  const a = gltf.accessors[index]
  const view = gltf.bufferViews[a.bufferView]
  const offset = view.byteOffset + (a.byteOffset ?? 0)
  const size = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 }[a.type]
  const Type = { 5121: Uint8Array, 5123: Uint16Array, 5125: Uint32Array, 5126: Float32Array }[a.componentType]
  if (!Type) throw new Error(`Componente GLTF não suportado: ${a.componentType}`)
  return new Type(binary.buffer, binary.byteOffset + offset, a.count * size)
}

function attribute(values, size) {
  const bytes = Buffer.from(values.buffer, values.byteOffset, values.byteLength)
  const offset = binary.length
  binary = Buffer.concat([binary, bytes, Buffer.alloc((4 - (bytes.length % 4)) % 4)])
  const bufferView = gltf.bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: bytes.length }) - 1
  const accessor = {
    bufferView,
    componentType: values instanceof Uint32Array ? 5125 : values instanceof Uint16Array ? 5123 : 5126,
    count: values.length / size,
    type: { 1: 'SCALAR', 2: 'VEC2', 3: 'VEC3', 4: 'VEC4', 16: 'MAT4' }[size],
  }
  if (size === 3) {
    accessor.min = [Infinity, Infinity, Infinity]
    accessor.max = [-Infinity, -Infinity, -Infinity]
    values.forEach((v, i) => {
      accessor.min[i % 3] = Math.min(accessor.min[i % 3], v)
      accessor.max[i % 3] = Math.max(accessor.max[i % 3], v)
    })
  }
  return gltf.accessors.push(accessor) - 1
}

function blob(png) {
  const bufferView = gltf.bufferViews.push({ buffer: 0, byteOffset: binary.length, byteLength: png.length }) - 1
  binary = Buffer.concat([binary, png, Buffer.alloc((4 - (png.length % 4)) % 4)])
  return bufferView
}
function texture(name, png) {
  const source = gltf.images.push({ name, mimeType: 'image/png', bufferView: blob(png) }) - 1
  return { index: gltf.textures.push({ source }) - 1 }
}
const imageBytes = (index) => {
  const view = gltf.bufferViews[gltf.images[index].bufferView]
  return Buffer.from(binary.subarray(view.byteOffset, view.byteOffset + view.byteLength))
}
/** Substitui a imagem em uso (mesmo índice), para que todo material que a referencia receba a nova. */
function replaceImage(name, png) {
  const image = gltf.images.find((entry) => entry.name === name)
  image.bufferView = blob(png)
  image.name = `Haru_${name}`
}
const imageIndex = (name) => gltf.images.findIndex((entry) => entry.name === name)
/** Morphs VRoid são esparsos (sem bufferView): lê denso, regrava só os vértices que se movem. */
const morphs = {
  read(index) {
    const a = gltf.accessors[index]
    const dense = a.bufferView === undefined ? new Float32Array(a.count * 3) : Float32Array.from(data(index))
    if (!a.sparse) return dense
    const view = (ref, Type, length) =>
      new Type(
        binary.buffer,
        binary.byteOffset + gltf.bufferViews[ref.bufferView].byteOffset + (ref.byteOffset ?? 0),
        length,
      )
    const ids = view(
      a.sparse.indices,
      { 5121: Uint8Array, 5123: Uint16Array, 5125: Uint32Array }[a.sparse.indices.componentType],
      a.sparse.count,
    )
    const values = view(a.sparse.values, Float32Array, a.sparse.count * 3)
    for (const [i, vertex] of ids.entries()) dense.set(values.subarray(i * 3, i * 3 + 3), vertex * 3)
    return dense
  },
  write(dense) {
    const ids = []
    for (let i = 0; i < dense.length / 3; i++) if (dense[i * 3] || dense[i * 3 + 1] || dense[i * 3 + 2]) ids.push(i)
    const min = [0, 1, 2].map((k) => Math.min(0, ...ids.map((i) => dense[i * 3 + k])))
    const max = [0, 1, 2].map((k) => Math.max(0, ...ids.map((i) => dense[i * 3 + k])))
    const accessor = { componentType: 5126, count: dense.length / 3, type: 'VEC3', min, max }
    if (ids.length) {
      const values = Float32Array.from(ids.flatMap((i) => [dense[i * 3], dense[i * 3 + 1], dense[i * 3 + 2]]))
      const bytes = (array) => blob(Buffer.from(array.buffer, array.byteOffset, array.byteLength))
      accessor.sparse = {
        count: ids.length,
        indices: { bufferView: bytes(Uint32Array.from(ids)), componentType: 5125 },
        values: { bufferView: bytes(values) },
      }
    }
    return gltf.accessors.push(accessor) - 1
  },
}
const material = (name, palette, options) => gltf.materials.push(toonMaterial(name, palette, options)) - 1

const clothMap = texture('Haru_WovenCloth', texturePng(512, 512, clothColor))
const scarfMap = texture('Haru_DiagonalSilk', texturePng(1024, 1024, scarfColor))
const foldMap = texture('Haru_BlazerFolds', texturePng(1024, 512, foldColor))
const ink = material('Haru_Blazer', PALETTE.blazer, { texture: foldMap, outline: 0.0011 })
const skirtMaterial = material('Haru_FittedSkirt', PALETTE.blazer, { texture: clothMap, outline: 0.0011 })
const pearl = material('Haru_Piping', PALETTE.piping, { outline: 0.0004 })
const shirtMaterial = material('Haru_Shirt', PALETTE.shirt, { outline: 0.0006 })
const scarf = material('Haru_ScarfSatin', ['#ffffff', '#b9c6dc'], { texture: scarfMap, outline: 0.0007, rim: 0.3 })
const stitch = material('Haru_Stitch', PALETTE.blazer, { outline: 0.0005 })
const bowMaterial = material('Haru_ShoeBow', PALETTE.bow, { outline: 0.0004 })

// Olhos: a íris VRoid (pupila, reflexos pintados) ganha o azul da Haru pela luminância.
replaceImage('EyeIris_00', recolorPng(imageBytes(imageIndex('EyeIris_00')), IRIS_STOPS))
replaceImage('Face_00', animeFacePng(imageBytes(imageIndex('Face_00'))))
replaceImage('FaceMouth_00', animeMouthPng(imageBytes(imageIndex('FaceMouth_00'))))
for (const name of ['Hair_00', 'HairBack_00']) {
  replaceImage(name, recolorPng(imageBytes(imageIndex(name)), HAIR_STOPS, hairStrokes))
}
const angelRing = texture('Haru_AngelRing', texturePng(256, 256, angelRingColor))
for (const mat of gltf.materials) {
  const toon = mat.extensions?.VRMC_materials_mtoon
  if (!toon) continue
  if (/SKIN$/.test(mat.name)) {
    inkOutline(mat, mat.name.startsWith('Face') ? 0.0007 : 0.0006)
    toon.shadeColorFactor = [0.94, 0.7, 0.7]
    // Rosto anime quase sempre iluminado; a sombra dura fica para o pescoço sob o queixo.
    if (mat.name.startsWith('Face')) toon.shadingShiftFactor = 0.35
  } else if (/HAIR$/.test(mat.name)) {
    inkOutline(mat, 0.0007)
    toon.shadeColorFactor = [0.62, 0.52, 0.72]
    toon.parametricRimColorFactor = [0.1, 0.08, 0.14]
    toon.parametricRimFresnelPowerFactor = 3.5
    toon.matcapTexture = angelRing
    toon.matcapFactor = [0.55, 0.52, 0.62]
    mat.emissiveFactor = [0, 0, 0]
    delete mat.emissiveTexture
  } else if (mat.name.startsWith('Shoes')) {
    Object.assign(mat, toonMaterial(mat.name, PALETTE.shoes, { outline: 0.0007, rim: 0.4 }))
  }
}
const eyeliner = gltf.materials.find((mat) => mat.name.startsWith('FaceEyeline'))
eyeliner.pbrMetallicRoughness.baseColorFactor = [0.32, 0.16, 0.22, 1]
const brow = gltf.materials.find((mat) => mat.name.startsWith('FaceBrow'))
brow.pbrMetallicRoughness.baseColorFactor = [0.2, 0.14, 0.24, 1]

function add(name, geometry, mat, bone, influence = bone === 'chest' ? torsoWeights : undefined) {
  const g = geometry
  const count = g.attributes.position.count
  if (!g.attributes.uv) {
    g.setAttribute(
      'uv',
      new T.Float32BufferAttribute(
        Array.from({ length: count * 2 }, (_, i) =>
          i % 2 === 0
            ? g.attributes.position.getX(Math.floor(i / 2)) * 4
            : g.attributes.position.getY(Math.floor(i / 2)) * 4,
        ),
        2,
      ),
    )
  }
  const joints = new Uint16Array(count * 4)
  const weights = new Float32Array(count * 4)
  for (let i = 0; i < count; i++) {
    const mix = influence?.(g.attributes.position, i) ?? [[bone, 1]]
    for (const [slot, [joint, weight]] of mix.entries()) {
      joints[i * 4 + slot] = skin.joints.indexOf(typeof joint === 'number' ? joint : bones[joint].node)
      weights[i * 4 + slot] = weight
    }
  }
  const mesh =
    gltf.meshes.push({
      name,
      primitives: [
        {
          material: mat,
          indices: attribute(
            g.index ? Uint32Array.from(g.index.array) : Uint32Array.from({ length: count }, (_, i) => i),
            1,
          ),
          attributes: {
            POSITION: attribute(g.attributes.position.array, 3),
            NORMAL: attribute(g.attributes.normal.array, 3),
            JOINTS_0: attribute(joints, 4),
            WEIGHTS_0: attribute(weights, 4),
            ...(g.attributes.uv ? { TEXCOORD_0: attribute(g.attributes.uv.array, 2) } : {}),
          },
        },
      ],
    }) - 1
  const node = gltf.nodes.push({ name, mesh, skin: 0 }) - 1
  gltf.scenes[gltf.scene ?? 0].nodes.push(node)
}

function tube(name, points, radius, mat, bone, influence, tapered = false) {
  let curve = new T.CatmullRomCurve3(points.map((p) => new T.Vector3(...p)))
  if (bone === 'chest' || bone === 'spine') {
    const surface = curve.getPoints(64).map(({ x, y }) => new T.Vector3(...garmentFront(x, y, 0.012)))
    curve = new T.CatmullRomCurve3(surface)
  }
  const geometry = new T.TubeGeometry(curve, 64, radius, 10, false)
  if (tapered) {
    const p = geometry.attributes.position
    for (let i = 0; i < p.count; i++) {
      const t = Math.floor(i / 11) / 64
      const center = curve.getPointAt(t)
      const point = new T.Vector3().fromBufferAttribute(p, i)
      point
        .sub(center)
        .multiplyScalar(0.98 * (1 - t ** 3) + 0.02)
        .multiplyScalar(name === 'Haru_SideCurlFace' ? T.MathUtils.smoothstep(t, 0, 0.16) : 1)
        .add(center)
      p.setXYZ(i, ...point.toArray())
    }
    geometry.computeVertexNormals()
  }
  add(name, geometry, mat, bone, influence)
}

function panel(name, points, mat, bone = 'chest') {
  if (bone === 'chest') {
    add(name, curvedPanel(T, points), mat, bone)
    return
  }
  const geometry = new T.BufferGeometry()
  const surface = bone === 'chest' ? points.map(([x, y]) => garmentFront(x, y, 0.006)) : points
  geometry.setAttribute('position', new T.Float32BufferAttribute(surface.flat(), 3))
  geometry.setIndex(
    T.ShapeUtils.triangulateShape(
      points.map((p) => new T.Vector2(p[0], p[1])),
      [],
    ).flat(),
  )
  geometry.computeVertexNormals()
  add(name, geometry, mat, bone)
}

// Chanel a partir das mechas VRoid: pontas afiladas e textura originais, dobradas até o queixo.
const hairPrimitive = gltf.meshes[2].primitives[0]
{
  warpInPlace(haruStylePoint, data(hairPrimitive.attributes.POSITION), data(hairPrimitive.attributes.NORMAL))
  const hairJoints = data(hairPrimitive.attributes.JOINTS_0)
  const hairWeights = data(hairPrimitive.attributes.WEIGHTS_0)
  for (let i = 0; i < hairJoints.length; i += 4) {
    hairJoints.set([gltf.skins[2].joints.indexOf(bones.head.node), 0, 0, 0], i)
    hairWeights.set([1, 0, 0, 0], i)
  }
  // As molas do cabelo longo ficariam penduradas sem vértice nenhum.
  const physics = gltf.extensions.VRMC_springBone
  physics.springs = physics.springs.filter((spring) => spring.name !== 'Hair')
}
const hair = gltf.materials.push({ ...structuredClone(gltf.materials[hairPrimitive.material]), name: 'Haru_Curls' }) - 1
// Retira só a camiseta; corpo, mãos, morphs e rig originais permanecem no arquivo.
gltf.meshes[0].primitives = gltf.meshes[0].primitives.filter((p) => p.material !== 3)
// Modela também a cintura da base, mantendo folga sob o tecido; shorts originais viram forro interno.
const wristBind = new T.Matrix4()
  .fromArray(data(skin.inverseBindMatrices), skin.joints.indexOf(bones.leftHand.node) * 16)
  .invert()
const coveredArmEnd = Math.abs(new T.Vector3().setFromMatrixPosition(wristBind).x) - 0.018
for (const primitive of gltf.meshes[0].primitives) {
  if (primitive.material !== 0 && primitive.material !== 1) continue
  const vertices = data(primitive.attributes.POSITION)
  for (let i = 0; i < vertices.length; i += 3) {
    const x = vertices[i],
      y = vertices[i + 1],
      z = vertices[i + 2]
    if (primitive.material === 1) {
      vertices[i] *= 0.55
      vertices[i + 2] *= 0.45
    } else if (y > 0.89 && y < 1.255 && Math.abs(x) < 0.17) {
      const [width, front, back] = torsoSection(y)
      const radius = Math.hypot(x / (width * 0.84), z / ((z > 0 ? front : back) * 0.8))
      if (radius > 1) {
        vertices[i] /= radius
        vertices[i + 2] /= radius
      }
    } else if (y > 0.5 && y < 0.89 && Math.abs(x) < 0.2) {
      const slim = 1 - T.MathUtils.smoothstep(y, 0.5, 0.72) * 0.2
      const legCenter = Math.sign(x) * 0.077
      vertices[i] = legCenter + (x - legCenter) * slim
      vertices[i + 2] *= slim
    }
  }
  if (primitive.material === 0) {
    // Faces inteiramente sob o uniforme não precisam disputar profundidade com a roupa durante gestos.
    const indices = data(primitive.indices)
    const visible = []
    for (let i = 0; i < indices.length; i += 3) {
      const triangle = [indices[i], indices[i + 1], indices[i + 2]]
      const covered = triangle.every((vertex) => {
        const offset = vertex * 3
        const y = vertices[offset + 1],
          x = Math.abs(vertices[offset])
        return (y > 0.685 && y < 1.335 && x < 0.2) || (y > 1.18 && y < 1.335 && x < coveredArmEnd)
      })
      if (!covered) visible.push(...triangle)
    }
    primitive.indices = attribute(Uint32Array.from(visible), 1)
  }
}

// Pernas esculpidas antes da subdivisão, para que a tesselação curva suavize joelho e panturrilha.
{
  const body = gltf.meshes[0].primitives.find((p) => p.material === 0)
  warpInPlace(sculptLeg, data(body.attributes.POSITION), data(body.attributes.NORMAL), isLeg)
}
// Blazer acinturado, saia e mangas com pesos reais do esqueleto.
for (const primitive of gltf.meshes[0].primitives) {
  if (primitive.material === 0 || primitive.material === 2) {
    subdividePrimitive(T, primitive, data, attribute, 2)
  }
}
// Densidade onde o contorno e o cel aparecem: silhueta do rosto, boca, mechas e sapatilhas.
for (const primitive of gltf.meshes[1].primitives) {
  if (/^(Face_00|FaceMouth)/.test(gltf.materials[primitive.material].name)) {
    subdividePrimitive(T, primitive, data, attribute, 1, morphs)
  }
}
subdividePrimitive(T, hairPrimitive, data, attribute, 1)
const blazer = tailoredTorso(T)
function torsoWeights(p, i) {
  const y = p.getY(i)
  const upper = T.MathUtils.smoothstep(y, 1.02, 1.22)
  const lower = 1 - T.MathUtils.smoothstep(y, 0.91, 1.06)
  const shoulder =
    T.MathUtils.smoothstep(Math.abs(p.getX(i)), 0.08, 0.135) *
    T.MathUtils.smoothstep(y, 1.2, 1.26) *
    (1 - T.MathUtils.smoothstep(y, 1.29, 1.33)) *
    0.95
  return [
    ['chest', upper * (1 - shoulder)],
    ['hips', lower * (1 - shoulder)],
    ['spine', (1 - upper - lower) * (1 - shoulder)],
    [p.getX(i) > 0 ? 'leftUpperArm' : 'rightUpperArm', shoulder],
  ]
}
add('Haru_Blazer', blazer, ink, 'chest', torsoWeights)
const matrices = data(skin.inverseBindMatrices)
function bonePosition(name) {
  const index = skin.joints.indexOf(bones[name].node)
  return new T.Vector3().setFromMatrixPosition(new T.Matrix4().fromArray(matrices, index * 16).invert())
}
const bindMatrices = Array.from(matrices)
function springChain(name, parent, points, stiffness) {
  let parentNode = bones[parent].node
  let inverse = new T.Matrix4().fromArray(matrices, skin.joints.indexOf(parentNode) * 16)
  const chain = points.map((point, i) => {
    const world = new T.Matrix4().makeTranslation(...point)
    const local = inverse.clone().multiply(world)
    const position = new T.Vector3(),
      rotation = new T.Quaternion(),
      scale = new T.Vector3()
    local.decompose(position, rotation, scale)
    const node =
      gltf.nodes.push({
        name: name + i,
        translation: position.toArray(),
        rotation: rotation.toArray(),
        scale: scale.toArray(),
        children: [],
      }) - 1
    gltf.nodes[parentNode].children ??= []
    gltf.nodes[parentNode].children.push(node)
    inverse = world.clone().invert()
    skin.joints.push(node)
    bindMatrices.push(...inverse.toArray())
    parentNode = node
    return { node, stiffness, dragForce: 0.65, gravityPower: 0.025, hitRadius: 0.008 }
  })
  gltf.extensions.VRMC_springBone.springs.push({ name, joints: chain })
  return chain.map((joint) => joint.node)
}
const curls = springChain(
  'Haru_CurlSpring',
  'head',
  [
    [-0.098, 1.4, -0.108],
    [-0.125, 1.3, -0.09],
    [-0.135, 1.2, -0.07],
  ],
  1.8,
)
// Oito painéis presos ao quadril; apenas a parte inferior da saia recebe inércia.
const skirtSprings = Array.from({ length: 8 }, (_, i) =>
  springChain(
    `Haru_SkirtSpring${i}`,
    'hips',
    [0.47, 0.75, 1].map((t) => skirtPoint((i * Math.PI) / 4, t)),
    4.8,
  ),
)
// A pencil skirt follows the thighs: hung only on springs, captured motion spread the legs through it.
// At most four influences, the slots a vertex has; a fifth used to spill into the next vertex.
function skirtWeights(p, i) {
  const length = (1.005 - p.getY(i)) / 0.475
  const loose = T.MathUtils.smoothstep(length, 0.3, 0.52)
  if (loose === 0) return [['hips', 1]]
  const sector = Math.round(((Math.atan2(p.getZ(i), p.getX(i)) / (2 * Math.PI) + 1) % 1) * 8) % 8
  const spring = skirtSprings[sector][T.MathUtils.smoothstep(length, 0.52, 0.84) > 0.5 ? 1 : 0]
  const left = T.MathUtils.smoothstep(p.getX(i), -0.035, 0.035)
  const leg = loose * 0.8
  return [
    ['hips', 1 - loose],
    ['leftUpperLeg', leg * left],
    ['rightUpperLeg', leg * (1 - left)],
    [spring, loose - leg],
  ].filter(([, weight]) => weight > 0)
}
add('Haru_Skirt', fittedSkirt(T), skirtMaterial, 'hips', skirtWeights)
const curlWeights = (p, i) => {
  const tip = 1 - T.MathUtils.smoothstep(p.getY(i), 1.22, 1.4)
  return [
    [curls[0], 1 - tip],
    [curls[1], tip],
  ]
}
for (const side of ['left', 'right']) {
  const shoulder = bonePosition(`${side}UpperArm`)
  const elbow = bonePosition(`${side}LowerArm`)
  const wrist = bonePosition(`${side}Hand`)
  const sleeve = tailoredSleeve(T, shoulder, elbow, wrist)
  add(`Haru_${side}TailoredSleeve`, sleeve.geometry, ink, `${side}UpperArm`, (p, i) => {
    const lower = sleeve.lowerWeight(p, i)
    return [
      [`${side}UpperArm`, 1 - lower],
      [`${side}LowerArm`, lower],
    ]
  })
  const cuff = new T.CylinderGeometry(0.029, 0.031, 0.019, 32, 2, true)
  cuff.applyQuaternion(
    new T.Quaternion().setFromUnitVectors(new T.Vector3(0, 1, 0), wrist.clone().sub(elbow).normalize()),
  )
  cuff.translate(...wrist.toArray())
  add('Haru_Cuff', cuff, pearl, `${side}LowerArm`)
}
const shirt = new T.PlaneGeometry(1, 1, 32, 64)
const shirtVertices = shirt.attributes.position
for (let i = 0; i < shirtVertices.count; i++) {
  const t = 0.5 - shirtVertices.getY(i)
  const shoulder = T.MathUtils.smoothstep(t, 0, 0.15)
  const halfWidth = T.MathUtils.lerp(0.025, 0.072 * (1 - t), shoulder)
  const x = shirtVertices.getX(i) * 2 * halfWidth
  const y = 1.325 - t * 0.285
  shirtVertices.setXYZ(i, ...garmentFront(x, y, 0.004 + Math.sin(t * Math.PI) * 0.0015 * Math.cos(x * 100)))
}
shirt.computeVertexNormals()
add('Haru_Shirt', shirt, shirtMaterial, 'chest')
for (const side of [-1, 1]) {
  const lapel = [
    [side * 0.063, 1.3, 0.115],
    [side * 0.115, 1.235, 0.113],
    [side * 0.075, 1.21, 0.131],
    [0, 1.035, 0.15],
  ]
  panel('Haru_Lapel', lapel, ink)
  tube('Haru_LapelPiping', lapel, 0.0015, pearl, 'chest')
}
tube(
  'Haru_CenterSeam',
  [
    [0, 1.035, 0.15],
    [0, 0.95, 0.139],
    [0, 0.9, 0.14],
    [0.004, 0.862, 0.148],
  ],
  0.0014,
  pearl,
  'chest',
)
panel(
  'Haru_Pocket',
  [
    [-0.12, 1.17, 0.092],
    [-0.06, 1.17, 0.125],
    [-0.06, 1.179, 0.125],
    [-0.12, 1.179, 0.092],
  ],
  pearl,
)
for (const y of [0.99, 0.935]) {
  add(
    'Haru_Button',
    new T.SphereGeometry(0.0062, 24, 12).scale(1, 1, 0.38).translate(...garmentFront(-0.014, y, 0.008)),
    stitch,
    'chest',
  )
}
// Seda contínua: listras diagonais nos UVs, nó e duas pontas com dobra e inércia próprias.
add('Haru_ScarfStripe', scarfWrap(T), scarf, 'neck')
add(
  'Haru_ScarfKnot',
  new T.SphereGeometry(1, 32, 24).scale(0.018, 0.015, 0.015).rotateZ(-0.4).translate(0.043, 1.329, 0.057),
  scarf,
  'neck',
)
for (const points of [
  [
    [0.043, 1.33, 0.069],
    [0.048, 1.307, 0.105],
    [0.066, 1.265, 0.145],
    [0.087, 1.23, 0.139],
  ],
  [
    [0.049, 1.332, 0.063],
    [0.075, 1.313, 0.096],
    [0.105, 1.285, 0.12],
    [0.133, 1.264, 0.092],
  ],
]) {
  const tailBones = springChain('Haru_ScarfSpring', 'neck', [points[0], points[2], points[3]], 3.5)
  add('Haru_ScarfTail', scarfTail(T, points, 0.023), scarf, 'neck', (_, vertex) => {
    const weight = T.MathUtils.smoothstep(Math.floor(vertex / 17) / 64, 0.15, 0.8)
    return [
      [tailBones[0], 1 - weight],
      [tailBones[1], weight],
    ]
  })
}
// Rabo baixo atrás da orelha direita, como o da Haru Live2D: dois cachos em espiral até o ombro.
for (const curl of [
  { top: [-0.098, 1.402, -0.108], bottom: [-0.138, 1.2, -0.068], radius: 0.034, turns: 4.2 },
  { top: [-0.084, 1.398, -0.122], bottom: [-0.108, 1.27, -0.118], radius: 0.024, turns: 3.2 },
]) {
  add('Haru_HairDrill', drillCurl(T, { ...curl, side: -1 }), hair, 'head', curlWeights)
}
add(
  'Haru_HairTie',
  new T.TorusGeometry(0.0145, 0.0065, 20, 64)
    .rotateX(Math.PI / 2 - 0.35)
    .rotateZ(0.5)
    .translate(-0.094, 1.41, -0.11),
  scarf,
  'head',
)
// Debrum claro no barrado do blazer e laço das sapatilhas.
{
  const loop = Array.from({ length: 160 }, (_, i) => {
    const angle = (i / 160) * Math.PI * 2
    const [width, front, back] = torsoSection(0.858)
    const z = Math.sin(angle)
    return new T.Vector3(Math.cos(angle) * (width + 0.0015), 0.858, z * ((z > 0 ? front : back) + 0.0015))
  })
  add(
    'Haru_BlazerHem',
    new T.TubeGeometry(new T.CatmullRomCurve3(loop, true), 320, 0.0022, 10, true),
    pearl,
    'chest',
    torsoWeights,
  )
}
// Sapatilhas: o tênis da base vira bico baixo e arredondado, com laço sobre o peito do pé.
const shoe = gltf.meshes[0].primitives.find((p) => gltf.materials[p.material].name.startsWith('Shoes'))
const shoeTop = { left: 0, right: 0 }
{
  const positions = data(shoe.attributes.POSITION)
  for (let i = 0; i < positions.length; i += 3) {
    const side = positions[i] > 0 ? 'left' : 'right'
    const center = bonePosition(`${side}Foot`).x
    const toe = T.MathUtils.smoothstep(positions[i + 2], 0.07, 0.135)
    positions[i] = center + (positions[i] - center) * (0.94 - toe * 0.14)
    // Garganta aberta: biqueira na altura original, peito do pé e calcanhar baixos.
    const cap = T.MathUtils.lerp(0.032, 0.2, T.MathUtils.smoothstep(positions[i + 2], 0.035, 0.085))
    positions[i + 1] = Math.min(0.012 + (positions[i + 1] - 0.012) * (1 - toe * 0.2), cap)
    if (Math.abs(positions[i + 2] - 0.07) < 0.01 && Math.abs(positions[i] - center) < 0.012) {
      shoeTop[side] = Math.max(shoeTop[side], positions[i + 1])
    }
  }
}
for (const side of ['left', 'right']) {
  const x = bonePosition(`${side}Foot`).x
  const y = shoeTop[side] + 0.004
  for (const lobe of [-1, 1]) {
    const loop = new T.SphereGeometry(1, 24, 12).scale(0.011, 0.0045, 0.0065).rotateZ(lobe * 0.35)
    add('Haru_ShoeBow', loop.translate(x + lobe * 0.0105, y, 0.07), bowMaterial, `${side}Foot`)
  }
  add('Haru_ShoeBow', new T.SphereGeometry(0.0042, 16, 10).translate(x, y + 0.001, 0.071), bowMaterial, `${side}Foot`)
}
// Costuras princesa e central: linhas de alfaiataria desenhadas pelo contorno bordô.
const seam = (points, surface) => {
  const curve = new T.CatmullRomCurve3(points.map(([x, y]) => new T.Vector3(...surface(x, y, 0.0035))))
  add('Haru_Seam', new T.TubeGeometry(curve, 96, 0.0009, 6, false), stitch, 'chest', torsoWeights)
}
seam(
  [
    [0, 1.31],
    [0, 1.1],
    [0, 0.87],
  ],
  garmentBack,
)
for (const side of [-1, 1]) {
  seam(
    [
      [side * 0.1, 1.29],
      [side * 0.062, 1.17],
      [side * 0.052, 1.05],
      [side * 0.07, 0.87],
    ],
    garmentBack,
  )
  seam(
    [
      [side * 0.098, 1.12],
      [side * 0.078, 1.05],
      [side * 0.088, 0.87],
    ],
    garmentFront,
  )
}
const hem = Array.from({ length: 97 }, (_, i) => {
  const angle = (i / 96) * Math.PI * 2
  return skirtPoint(angle, 1)
})
tube('Haru_SkirtHem', hem, 0.0017, pearl, 'hips', skirtWeights)
for (const side of [-1, 1]) {
  const collar = [
    [side * 0.025, 1.325, 0.058],
    [side * 0.078, 1.287, 0.12],
    [side * 0.055, 1.25, 0.12],
  ]
  add('Haru_FoldedCollar', curvedPanel(T, collar, 0.008), pearl, 'chest')
  tube(
    'Haru_PocketStitch',
    [
      [side * 0.035, 0.982, 0.122],
      [side * 0.08, 0.995, 0.106],
      [side * 0.097, 1.002, 0.084],
    ],
    0.0012,
    stitch,
    'spine',
  )
}
addOutfits({ T, add, material, clothMap, skirtWeights, hair: data(hairPrimitive.attributes.POSITION) })
skin.inverseBindMatrices = attribute(new Float32Array(bindMatrices), 16)
// Colisores locais do tronco e das coxas, compartilhados pelas peças que realmente os tocam.
const physics = gltf.extensions.VRMC_springBone
function collider(name, bone, from, to, radius) {
  const inverse = new T.Matrix4().fromArray(bindMatrices, skin.joints.indexOf(bones[bone].node) * 16)
  const local = (p) => new T.Vector3(...p).applyMatrix4(inverse).toArray()
  const index =
    physics.colliders.push({
      node: bones[bone].node,
      shape: { capsule: { offset: local(from), tail: local(to), radius } },
    }) - 1
  return physics.colliderGroups.push({ name, colliders: [index] }) - 1
}
const torsoCollider = collider('Haru_TorsoCollider', 'chest', [0, 1.14, 0.025], [0, 1.235, 0.055], 0.085)
const headCollider = collider('Haru_HeadCollider', 'head', [0, 1.42, 0], [0, 1.5, 0], 0.075)
const thighs = [-1, 1].map((side) =>
  collider(
    'Haru_ThighCollider',
    side > 0 ? 'leftUpperLeg' : 'rightUpperLeg',
    [side * 0.077, 0.84, 0],
    [side * 0.077, 0.69, 0],
    0.043,
  ),
)
for (const spring of physics.springs.filter((s) => s.name?.startsWith('Haru_'))) {
  spring.colliderGroups = spring.name.startsWith('Haru_Skirt') ? thighs : [torsoCollider, headCollider]
  for (const joint of spring.joints) {
    joint.dragForce = spring.name.startsWith('Haru_Skirt') ? 0.65 : 0.48
    joint.hitRadius = spring.name.startsWith('Haru_Skirt') ? 0.006 : 0.01
    joint.gravityPower = spring.name.startsWith('Haru_Skirt') ? 0.08 : 0.035
  }
}
// Legwear per look over bare legs: tights for the Haru look, socks and thigh-highs for the outfits.
{
  const primitive = gltf.meshes[0].primitives.find((p) => p.material === 0)
  const legs = {
    positions: data(primitive.attributes.POSITION),
    normals: data(primitive.attributes.NORMAL),
    joints: data(primitive.attributes.JOINTS_0),
    weights: data(primitive.attributes.WEIGHTS_0),
    indices: data(primitive.indices),
  }
  // One vertex buffer for every legwear; each look's mesh only picks the triangles below its top.
  const shell = legShell(T, legs, 0.86)
  add(
    'Haru_Tights',
    shell.geometry,
    material('Haru_Tights', ['#3b2e46', '#1d1627'], { outline: 0.0007, rim: 0.35 }),
    'hips',
    (_, i) => shell.mix[i].map(([joint, w]) => [skin.joints[joint], w]),
  )
  const tights = gltf.meshes.at(-1).primitives[0]
  tights.indices = attribute(Uint16Array.from(shell.geometry.index.array), 1)
  const legwear = (name, top, palette, cuff) => {
    const indices = belowHeight(shell.geometry, top)
    const mesh =
      gltf.meshes.push({
        name,
        primitives: [
          {
            ...tights,
            attributes: { ...tights.attributes },
            material: material(name, palette, { outline: 0.0007, rim: 0.35 }),
            indices: attribute(Uint16Array.from(indices), 1),
          },
        ],
      }) - 1
    gltf.scenes[gltf.scene ?? 0].nodes.push(gltf.nodes.push({ name, mesh, skin: 0 }) - 1)
    const cuffMat = material(`${name}Cuff`, cuff, { outline: 0.0006 })
    for (const side of [1, -1]) {
      const leg = `${side > 0 ? 'left' : 'right'}${top > 0.5 ? 'Upper' : 'Lower'}Leg`
      tube(name, cuffRing(legRadius(legs, side, top - 0.004)), 0.0045, cuffMat, leg)
    }
  }
  legwear('Outfit_Seifuku_Socks', 0.44, ['#26232f', '#100e16'], ['#26232f', '#100e16'])
  legwear('Outfit_Maid_Stockings', 0.6, ['#f7f5fb', '#bab5cd'], ['#2b2735', '#121019'])
  legwear('Outfit_Idol_Stockings', 0.62, ['#fbf7fb', '#cdbcd3'], ['#ff7fb2', '#c9447e'])
}
replaceImage('Body_00', recodePng(imageBytes(imageIndex('Body_00'))))
refineAnimeProportions(T, gltf, binary)

gltf.extensions.VRMC_vrm.meta.name = 'Mika Haru — Celestial Greeter'
gltf.extensions.VRMC_vrm.meta.version = '3.0.0'
gltf.extensions.VRMC_vrm.meta.authors.push('Mika project — procedural outfit and hairstyle')
const thumbnail = readFileSync(new URL('preview-haru.png', folder))
gltf.images[gltf.extensions.VRMC_vrm.meta.thumbnailImage] = {
  mimeType: 'image/png',
  bufferView: gltf.bufferViews.push({ buffer: 0, byteOffset: binary.length, byteLength: thumbnail.length }) - 1,
}
binary = Buffer.concat([binary, thumbnail, Buffer.alloc((4 - (thumbnail.length % 4)) % 4)])
binary = compactGltf(gltf, binary)
const json = Buffer.from(JSON.stringify(gltf))
const padded = Buffer.concat([json, Buffer.alloc((4 - (json.length % 4)) % 4, 32)])
const header = Buffer.alloc(20)
header.writeUInt32LE(0x46546c67, 0)
header.writeUInt32LE(2, 4)
header.writeUInt32LE(28 + padded.length + binary.length, 8)
header.writeUInt32LE(padded.length, 12)
header.writeUInt32LE(0x4e4f534a, 16)
const binHeader = Buffer.alloc(8)
binHeader.writeUInt32LE(binary.length, 0)
binHeader.writeUInt32LE(0x004e4942, 4)
writeFileSync(new URL('haru.vrm', folder), Buffer.concat([header, padded, binHeader, binary]))
