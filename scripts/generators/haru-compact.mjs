/**
 * O gerador só anexa bytes: cada nível de subdivisão e cada textura substituída deixa a versão
 * anterior órfã no binário. Reescreve o buffer com o que ainda é alcançável a partir do glTF.
 */
function collectPrimitiveAccessors(primitive, usedAccessors) {
  for (const index of Object.values(primitive.attributes)) usedAccessors.add(index)
  if (primitive.indices !== undefined) usedAccessors.add(primitive.indices)
  for (const target of primitive.targets ?? []) for (const index of Object.values(target)) usedAccessors.add(index)
}

function collectUsedAccessors(gltf) {
  const usedAccessors = new Set()
  for (const mesh of gltf.meshes) {
    for (const primitive of mesh.primitives) collectPrimitiveAccessors(primitive, usedAccessors)
  }
  for (const skin of gltf.skins ?? [])
    if (skin.inverseBindMatrices !== undefined) usedAccessors.add(skin.inverseBindMatrices)
  for (const animation of gltf.animations ?? []) {
    for (const sampler of animation.samplers) usedAccessors.add(sampler.input).add(sampler.output)
  }
  return usedAccessors
}

function collectViewRefs(gltf, accessors) {
  const viewRefs = []
  for (const accessor of accessors) {
    if (accessor.bufferView !== undefined) viewRefs.push([accessor, 'bufferView'])
    if (accessor.sparse) viewRefs.push([accessor.sparse.indices, 'bufferView'], [accessor.sparse.values, 'bufferView'])
  }
  for (const image of gltf.images ?? []) if (image.bufferView !== undefined) viewRefs.push([image, 'bufferView'])
  return viewRefs
}

function repackBufferViews(gltf, binary, viewRefs) {
  const views = []
  const viewMap = new Map()
  const chunks = []
  let length = 0
  for (const [owner, key] of viewRefs) {
    const old = owner[key]
    if (!viewMap.has(old)) {
      const view = gltf.bufferViews[old]
      const bytes = binary.subarray(view.byteOffset ?? 0, (view.byteOffset ?? 0) + view.byteLength)
      const padding = Buffer.alloc((4 - (bytes.length % 4)) % 4)
      viewMap.set(old, views.push({ ...view, byteOffset: length }) - 1)
      chunks.push(bytes, padding)
      length += bytes.length + padding.length
    }
    owner[key] = viewMap.get(old)
  }
  return { views, chunks }
}

function remapPrimitiveIndices(primitive, remap) {
  for (const key of Object.keys(primitive.attributes)) primitive.attributes[key] = remap(primitive.attributes[key])
  if (primitive.indices !== undefined) primitive.indices = remap(primitive.indices)
  for (const target of primitive.targets ?? []) for (const key of Object.keys(target)) target[key] = remap(target[key])
}

function remapAccessorIndices(gltf, remap) {
  for (const mesh of gltf.meshes) {
    for (const primitive of mesh.primitives) remapPrimitiveIndices(primitive, remap)
  }
  for (const skin of gltf.skins ?? [])
    if (skin.inverseBindMatrices !== undefined) skin.inverseBindMatrices = remap(skin.inverseBindMatrices)
  for (const animation of gltf.animations ?? []) {
    for (const sampler of animation.samplers) {
      sampler.input = remap(sampler.input)
      sampler.output = remap(sampler.output)
    }
  }
}

export function compactGltf(gltf, binary) {
  const usedAccessors = collectUsedAccessors(gltf)
  const accessorMap = new Map([...usedAccessors].sort((a, b) => a - b).map((old, next) => [old, next]))
  const accessors = [...accessorMap.keys()].map((old) => gltf.accessors[old])

  const viewRefs = collectViewRefs(gltf, accessors)
  const { views, chunks } = repackBufferViews(gltf, binary, viewRefs)

  remapAccessorIndices(gltf, (index) => accessorMap.get(index))

  gltf.accessors = accessors
  gltf.bufferViews = views
  const compacted = Buffer.concat(chunks)
  gltf.buffers[0].byteLength = compacted.length
  return compacted
}
