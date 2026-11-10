/**
 * Legwear as its own skin-weighted shells over the legs, so each look picks tights, knee socks or
 * thigh-highs. Painted into the body texture, one legwear was forced on every outfit, and a bare
 * band of skin between skirt and sock is what reads as anime.
 */
const OFFSET = 0.0016

/** Body leg triangles below `top`, pushed out along the normal, with the body's own skin weights. */
export function legShell(T, body, top) {
  const { positions, normals, joints, weights, indices } = body
  const remap = new Map()
  const position = []
  const normal = []
  const mix = []
  const index = []
  const vertex = (v) => {
    if (!remap.has(v)) {
      remap.set(v, position.length / 3)
      for (let k = 0; k < 3; k++) {
        normal.push(normals[v * 3 + k])
        position.push(positions[v * 3 + k] + normals[v * 3 + k] * OFFSET)
      }
      mix.push([0, 1, 2, 3].map((k) => [joints[v * 4 + k], weights[v * 4 + k]]).filter(([, w]) => w > 0))
    }
    return remap.get(v)
  }
  for (let i = 0; i < indices.length; i += 3) {
    const tri = [indices[i], indices[i + 1], indices[i + 2]]
    const ys = tri.map((v) => positions[v * 3 + 1])
    const xs = tri.map((v) => Math.abs(positions[v * 3]))
    if ((ys[0] + ys[1] + ys[2]) / 3 > top || Math.max(...xs) > 0.2) continue
    index.push(...tri.map(vertex))
  }
  const geometry = new T.BufferGeometry()
  geometry.setAttribute('position', new T.Float32BufferAttribute(position, 3))
  geometry.setAttribute('normal', new T.Float32BufferAttribute(normal, 3))
  geometry.setIndex(index)
  return { geometry, mix }
}

/** Triangles of `geometry` whose centroid sits below `top`. */
export function belowHeight(geometry, top) {
  const index = geometry.index.array
  const y = (v) => geometry.attributes.position.getY(v)
  const kept = []
  for (let i = 0; i < index.length; i += 3) {
    if ((y(index[i]) + y(index[i + 1]) + y(index[i + 2])) / 3 < top) kept.push(index[i], index[i + 1], index[i + 2])
  }
  return Uint32Array.from(kept)
}

/** Average leg radius around one leg's axis at height `y`, measured on the body itself. */
export function legRadius(body, side, y) {
  const { positions } = body
  let sum = 0
  let count = 0
  let cx = 0
  let cz = 0
  const near = []
  for (let v = 0; v < positions.length / 3; v++) {
    const [x, py, z] = [positions[v * 3], positions[v * 3 + 1], positions[v * 3 + 2]]
    if (Math.abs(py - y) > 0.006 || Math.sign(x) !== side || Math.abs(x) > 0.2) continue
    near.push([x, z])
    cx += x
    cz += z
  }
  cx /= near.length
  cz /= near.length
  for (const [x, z] of near) {
    sum += Math.hypot(x - cx, z - cz)
    count++
  }
  return { center: [cx, y, cz], radius: sum / count }
}

/** Ring of `points` around a leg at the shell's top edge: the sock cuff that hides the cut. */
export function cuffRing({ center: [cx, y, cz], radius }, lift = 0.004) {
  return Array.from({ length: 33 }, (_, i) => {
    const a = (i / 32) * Math.PI * 2
    return [cx + Math.sin(a) * (radius + lift), y, cz + Math.cos(a) * (radius + lift)]
  })
}
