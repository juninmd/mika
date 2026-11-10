/**
 * Tesselação curva em espaço de bind: conserva UVs e mistura os quatro pesos mais relevantes.
 * Morphs (expressões) são interpolados linearmente e regravados esparsos via `morphs`.
 */
export function subdividePrimitive(T, primitive, data, attribute, levels = 2, morphs) {
  if (primitive.targets?.length && !morphs) throw new Error('Primitiva com morphs exige leitor/escritor esparso')
  for (let level = 0; level < levels; level++) subdivideLevel(T, primitive, data, attribute, morphs)
}

function subdivideLevel(T, primitive, data, attribute, morphs) {
  const count = data(primitive.attributes.POSITION).length / 3
  const targets = (primitive.targets ?? []).map((target) =>
    Object.fromEntries(Object.entries(target).map(([key, index]) => [key, Array.from(morphs.read(index))])),
  )
  const attributes = Object.fromEntries(
    Object.entries(primitive.attributes).map(([key, index]) => {
      const values = data(index)
      return [key, { values: Array.from(values), size: values.length / count }]
    }),
  )
  const original = data(primitive.indices)
  const edges = new Map(),
    uses = new Map(),
    indices = []
  const edgeKey = (a, b) => (a < b ? `${a}:${b}` : `${b}:${a}`)
  for (let i = 0; i < original.length; i += 3) {
    for (const [a, b] of [
      [original[i], original[i + 1]],
      [original[i + 1], original[i + 2]],
      [original[i + 2], original[i]],
    ]) {
      const key = edgeKey(a, b)
      uses.set(key, (uses.get(key) ?? 0) + 1)
    }
  }
  const midpoint = (a, b) => {
    const key = edgeKey(a, b)
    if (edges.has(key)) return edges.get(key)
    const index = attributes.POSITION.values.length / 3
    const point = curvedMidpoint(T, attributes, a, b, uses.get(key) === 2)
    const skin = blendSkin(attributes, a, b)
    pushBlendedAttributes(T, attributes, a, b, point, skin)
    pushBlendedTargets(targets, a, b)
    edges.set(key, index)
    return index
  }
  for (let i = 0; i < original.length; i += 3) {
    const a = original[i],
      b = original[i + 1],
      c = original[i + 2]
    const ab = midpoint(a, b),
      bc = midpoint(b, c),
      ca = midpoint(c, a)
    indices.push(a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca)
  }
  for (const [name, entry] of Object.entries(attributes)) {
    const Type = name.startsWith('JOINTS') ? Uint16Array : Float32Array
    primitive.attributes[name] = attribute(Type.from(entry.values), entry.size)
  }
  primitive.indices = attribute(Uint32Array.from(indices), 1)
  primitive.targets = targets.map((target) =>
    Object.fromEntries(Object.entries(target).map(([key, values]) => [key, morphs.write(Float32Array.from(values))])),
  )
  if (!targets.length) delete primitive.targets
}

function pushBlendedAttributes(T, attributes, a, b, point, skin) {
  for (const [name, entry] of Object.entries(attributes)) {
    let value = Array.from(
      { length: entry.size },
      (_, j) => (entry.values[a * entry.size + j] + entry.values[b * entry.size + j]) * 0.5,
    )
    if (name === 'POSITION') value = point.toArray()
    if (name === 'NORMAL') value = new T.Vector3(...value).normalize().toArray()
    if (name === 'JOINTS_0') value = skin.map(([joint]) => joint)
    if (name === 'WEIGHTS_0') value = skin.map(([, weight]) => weight)
    entry.values.push(...value)
  }
}

function pushBlendedTargets(targets, a, b) {
  for (const target of targets) {
    for (const values of Object.values(target)) {
      for (let j = 0; j < 3; j++) values.push((values[a * 3 + j] + values[b * 3 + j]) * 0.5)
    }
  }
}

function curvedMidpoint(T, attributes, a, b, smooth) {
  const position = attributes.POSITION.values,
    normal = attributes.NORMAL.values
  const start = new T.Vector3().fromArray(position, a * 3)
  const end = new T.Vector3().fromArray(position, b * 3)
  const mid = start.clone().lerp(end, 0.5)
  if (!smooth) return mid
  const n1 = new T.Vector3().fromArray(normal, a * 3).normalize()
  const n2 = new T.Vector3().fromArray(normal, b * 3).normalize()
  const correction = n1
    .multiplyScalar(mid.clone().sub(start).dot(n1))
    .add(n2.multiplyScalar(mid.clone().sub(end).dot(n2)))
    .multiplyScalar(-0.375)
  // Evita saliências em arestas de normais muito divergentes, como unhas e sola.
  correction.clampLength(0, start.distanceTo(end) * 0.15)
  return mid.add(correction)
}

function blendSkin(attributes, a, b) {
  const joints = attributes.JOINTS_0.values,
    weights = attributes.WEIGHTS_0.values
  const combined = new Map()
  for (const vertex of [a, b]) {
    for (let slot = 0; slot < 4; slot++) {
      const index = vertex * 4 + slot,
        joint = joints[index]
      combined.set(joint, (combined.get(joint) ?? 0) + weights[index] * 0.5)
    }
  }
  const result = [...combined].sort((a, b) => b[1] - a[1]).slice(0, 4)
  const total = result.reduce((sum, [, weight]) => sum + weight, 0)
  while (result.length < 4) result.push([0, 0])
  return result.map(([joint, weight]) => [joint, weight / total])
}
