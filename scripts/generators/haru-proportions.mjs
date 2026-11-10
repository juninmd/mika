function warpMeshes(T, gltf, io, shape) {
  const touched = new Set()
  for (const mesh of gltf.meshes) {
    for (const primitive of mesh.primitives) {
      if (touched.has(primitive.attributes.POSITION)) continue
      touched.add(primitive.attributes.POSITION)
      const material = gltf.materials[primitive.material]?.name ?? ''
      const face = mesh.name === 'Face'
      const head = face || /^Hair/.test(material) || /^(Haru_(SideCurl|Hair)|Outfit_\w+_Head)/.test(mesh.name)
      const skin = /^(Body|Haru_Tights|Outfit_\w+_(Socks|Stockings))$/.test(mesh.name)
      warpPrimitive(T, io, primitive, (point) => shape(point, head, face, skin))
    }
  }
}

function warpSpringBoneColliders(T, gltf, worlds, nextWorlds, headNodes, shape) {
  for (const collider of gltf.extensions.VRMC_springBone?.colliders ?? []) {
    const volume = collider.shape.sphere ?? collider.shape.capsule
    const inverse = nextWorlds[collider.node].clone().invert()
    for (const key of ['offset', 'tail']) {
      if (!volume[key]) continue
      const point = new T.Vector3(...volume[key]).applyMatrix4(worlds[collider.node])
      volume[key] = shape(point, headNodes.has(collider.node)).applyMatrix4(inverse).toArray()
    }
    if (headNodes.has(collider.node)) volume.radius *= 0.93
  }
}

function warpLookAtOffset(T, gltf, worlds, nextWorlds, humanoid, shape) {
  const look = gltf.extensions.VRMC_vrm.lookAt
  if (!look?.offsetFromHeadBone) return
  const node = humanoid.head.node
  const point = new T.Vector3(...look.offsetFromHeadBone).applyMatrix4(worlds[node])
  look.offsetFromHeadBone = shape(point, true).applyMatrix4(nextWorlds[node].clone().invert()).toArray()
}

/** Redesenho em espaço de bind, incluindo esqueleto e morphs; não depende de escala no renderer. */
export function refineAnimeProportions(T, gltf, binary) {
  const io = accessorIO(gltf, binary)
  const worlds = nodeWorlds(T, gltf.nodes)
  const humanoid = gltf.extensions.VRMC_vrm.humanoid.humanBones
  const headNodes = descendants(gltf.nodes, humanoid.head.node)
  const wrists = ['leftHand', 'rightHand'].map((name) =>
    new T.Vector3().setFromMatrixPosition(worlds[humanoid[name].node]),
  )
  const shape = (point, head = false, face = false, skin = false) => reshape(T, point, wrists, head, face, skin)
  warpMeshes(T, gltf, io, shape)
  const nextWorlds = worlds.map((world, node) =>
    world.clone().setPosition(shape(new T.Vector3().setFromMatrixPosition(world), headNodes.has(node))),
  )
  moveSkeleton(T, gltf, io, nextWorlds)
  warpSpringBoneColliders(T, gltf, worlds, nextWorlds, headNodes, shape)
  warpLookAtOffset(T, gltf, worlds, nextWorlds, humanoid, shape)
}

function reshape(T, point, wrists, head, face, skin) {
  const p = point.clone()
  if (face) refineFace(T, p)
  if (skin) shapeCalf(p)
  const headWeight = head ? 1 : T.MathUtils.smoothstep(point.y, 1.33, 1.37)
  p.x *= 1 - 0.1 * headWeight
  p.y = 1.36 + (p.y - 1.36) * (1 - 0.1 * headWeight)
  p.z *= 1 - 0.09 * headWeight
  for (const wrist of wrists) {
    const side = Math.sign(wrist.x)
    const weight = T.MathUtils.smoothstep(point.x * side, Math.abs(wrist.x) - 0.025, Math.abs(wrist.x) + 0.025)
    if (weight > 0)
      p.sub(wrist)
        .multiplyScalar(1 - weight * 0.16)
        .add(wrist)
  }
  p.y = balancedHeight(T, p.y)
  return p
}

/**
 * Alonga só a canela, sem tocar sapatos: pernas a ~55% da altura, como heroína anime adulta.
 * A versão anterior encurtava as pernas e mantinha os braços, que passavam a parecer compridos.
 */
function balancedHeight(T, y) {
  return kneeAtMidLeg(y + 0.03 * T.MathUtils.smoothstep(y, 0.12, 0.515))
}

const [ANKLE, KNEE, CROTCH, ANIME_KNEE] = [0.101, 0.545, 0.898, 0.5]

/** Anime legs have thigh ≈ calf; the VRoid base had a 0.35 thigh over a 0.44 calf. */
function kneeAtMidLeg(y) {
  if (y <= ANKLE || y >= CROTCH) return y
  if (y <= KNEE) return ANKLE + ((y - ANKLE) * (ANIME_KNEE - ANKLE)) / (KNEE - ANKLE)
  return ANIME_KNEE + ((y - KNEE) * (CROTCH - ANIME_KNEE)) / (CROTCH - KNEE)
}

const bump = (y, center, width) => Math.exp(-(((y - center) / width) ** 2))

/**
 * Below the skirt the base leg is a straight tube. Anime legs swell at the calf and taper to a slim
 * ankle; the thigh stays as is so it never pushes through the pencil skirt.
 */
function shapeCalf(p) {
  if (Math.abs(p.x) > 0.16 || p.y > 0.56 || p.y < 0.13) return
  const axisX = Math.sign(p.x) * 0.077
  const axisZ = -0.007 - ((0.5 - p.y) / 0.4) * 0.025
  const scale = 1 + 0.06 * bump(p.y, 0.38, 0.075) - 0.1 * bump(p.y, 0.19, 0.035) - 0.02 * bump(p.y, 0.5, 0.03)
  p.x = axisX + (p.x - axisX) * scale
  p.z = axisZ + (p.z - axisZ) * scale
}

function refineFace(T, p) {
  const front = T.MathUtils.smoothstep(p.z, 0.01, 0.05)
  const jaw = Math.exp(-(((p.y - 1.39) / 0.035) ** 2)) * front
  p.x *= 1 - jaw * 0.045
  const orbit =
    (1 - T.MathUtils.smoothstep(Math.abs(Math.abs(p.x) - 0.043), 0.021, 0.038)) *
    (1 - T.MathUtils.smoothstep(Math.abs(p.y - 1.444), 0.019, 0.04)) *
    front
  p.y += ((p.y - 1.444) * -0.24 + (Math.abs(p.x) - 0.043) * 0.1) * orbit
  p.x += Math.sign(p.x) * (Math.abs(p.x) - 0.043) * orbit * 0.1
  p.z += Math.exp(-((p.x / 0.011) ** 2) - ((p.y - Math.SQRT2) / 0.014) ** 2) * front * 0.004
  p.z += Math.exp(-((p.x / 0.014) ** 2) - ((p.y - 1.3845) / 0.002) ** 2) * front * 0.0006
  // Reequilibra o espaço entre olhos, testa e queixo, sem deslocar o crânio nem as orelhas.
  p.y += Math.exp(-(((p.y - 1.434) / 0.058) ** 2)) * front * 0.012
}

function normalAt(T, warp, position, normal) {
  const epsilon = 0.00002
  const origin = warp(position)
  const columns = [
    [epsilon, 0, 0],
    [0, epsilon, 0],
    [0, 0, epsilon],
  ].map((offset) =>
    warp(position.clone().add(new T.Vector3(...offset)))
      .sub(origin)
      .multiplyScalar(1 / epsilon),
  )
  const jacobian = new T.Matrix3().set(
    columns[0].x,
    columns[1].x,
    columns[2].x,
    columns[0].y,
    columns[1].y,
    columns[2].y,
    columns[0].z,
    columns[1].z,
    columns[2].z,
  )
  return normal.clone().applyMatrix3(jacobian.invert().transpose()).normalize()
}

function warpPrimitive(T, io, primitive, warp) {
  const positions = io.read(primitive.attributes.POSITION)
  const normals = io.read(primitive.attributes.NORMAL)
  const moved = new Float32Array(positions.length)
  const shaded = new Float32Array(normals.length)
  for (let i = 0; i < positions.length; i += 3) {
    const position = new T.Vector3().fromArray(positions, i)
    warp(position).toArray(moved, i)
    normalAt(T, warp, position, new T.Vector3().fromArray(normals, i)).toArray(shaded, i)
  }
  for (const target of primitive.targets ?? []) {
    warpTarget(T, io, target, positions, normals, moved, shaded, warp)
  }
  io.write(primitive.attributes.POSITION, moved)
  io.write(primitive.attributes.NORMAL, shaded)
}

function warpTarget(T, io, target, positions, normals, moved, shaded, warp) {
  for (const key of ['POSITION', 'NORMAL']) {
    if (target[key] === undefined) continue
    const delta = io.read(target[key])
    for (let i = 0; i < delta.length; i += 3) {
      if (delta[i] === 0 && delta[i + 1] === 0 && delta[i + 2] === 0) continue
      const position = new T.Vector3().fromArray(positions, i)
      const change = new T.Vector3().fromArray(delta, i)
      const result =
        key === 'POSITION'
          ? warp(position.add(change))
          : normalAt(T, warp, position, new T.Vector3().fromArray(normals, i).add(change))
      result.sub(new T.Vector3().fromArray(key === 'POSITION' ? moved : shaded, i)).toArray(delta, i)
    }
    io.write(target[key], delta)
  }
}

function accessorIO(gltf, binary) {
  const view = (bufferView, offset, Type, length) =>
    new Type(binary.buffer, binary.byteOffset + (gltf.bufferViews[bufferView].byteOffset ?? 0) + (offset ?? 0), length)
  const sparseIndices = (sparse) =>
    view(
      sparse.indices.bufferView,
      sparse.indices.byteOffset,
      { 5121: Uint8Array, 5123: Uint16Array, 5125: Uint32Array }[sparse.indices.componentType],
      sparse.count,
    )
  return {
    read(index) {
      const a = gltf.accessors[index],
        size = a.type === 'MAT4' ? 16 : 3
      const result =
        a.bufferView === undefined
          ? new Float32Array(a.count * size)
          : Float32Array.from(view(a.bufferView, a.byteOffset, Float32Array, a.count * size))
      if (a.sparse) {
        const values = view(a.sparse.values.bufferView, a.sparse.values.byteOffset, Float32Array, a.sparse.count * size)
        for (const [i, vertex] of sparseIndices(a.sparse).entries()) {
          result.set(values.subarray(i * size, (i + 1) * size), vertex * size)
        }
      }
      return result
    },
    write(index, values) {
      const a = gltf.accessors[index],
        size = a.type === 'MAT4' ? 16 : 3
      if (a.bufferView !== undefined) view(a.bufferView, a.byteOffset, Float32Array, values.length).set(values)
      if (a.sparse) {
        const target = view(a.sparse.values.bufferView, a.sparse.values.byteOffset, Float32Array, a.sparse.count * size)
        for (const [i, vertex] of sparseIndices(a.sparse).entries()) {
          target.set(values.subarray(vertex * size, (vertex + 1) * size), i * size)
        }
      }
      if (a.min || a.max) {
        a.min = Array(size).fill(Infinity)
        a.max = Array(size).fill(-Infinity)
        values.forEach((v, i) => {
          a.min[i % size] = Math.min(a.min[i % size], v)
          a.max[i % size] = Math.max(a.max[i % size], v)
        })
      }
    },
  }
}

function nodeWorlds(T, nodes) {
  const parents = new Map()
  nodes.forEach((node, index) => {
    for (const child of node.children ?? []) parents.set(child, index)
  })
  const cache = new Map()
  const world = (index) => {
    if (cache.has(index)) return cache.get(index)
    const n = nodes[index]
    const local = n.matrix
      ? new T.Matrix4().fromArray(n.matrix)
      : new T.Matrix4().compose(
          new T.Vector3(...(n.translation ?? [0, 0, 0])),
          new T.Quaternion(...(n.rotation ?? [0, 0, 0, 1])),
          new T.Vector3(...(n.scale ?? [1, 1, 1])),
        )
    const result = parents.has(index) ? world(parents.get(index)).clone().multiply(local) : local
    cache.set(index, result)
    return result
  }
  return nodes.map((_, index) => world(index))
}

function descendants(nodes, root) {
  const found = new Set([root])
  for (const node of found) for (const child of nodes[node].children ?? []) found.add(child)
  return found
}

function moveSkeleton(T, gltf, io, worlds) {
  const parents = new Map()
  gltf.nodes.forEach((node, index) => {
    for (const child of node.children ?? []) parents.set(child, index)
  })
  gltf.nodes.forEach((node, index) => {
    const local = parents.has(index)
      ? worlds[parents.get(index)].clone().invert().multiply(worlds[index])
      : worlds[index]
    if (node.matrix) node.matrix = local.toArray()
    else node.translation = new T.Vector3().setFromMatrixPosition(local).toArray()
  })
  for (const skin of gltf.skins) {
    const matrices = Float32Array.from(skin.joints.flatMap((node) => worlds[node].clone().invert().toArray()))
    io.write(skin.inverseBindMatrices, matrices)
  }
}
