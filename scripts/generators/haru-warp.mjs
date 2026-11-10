/** Deformações em espaço de bind: a normal acompanha pelo jacobiano numérico, senão o cel quebra. */

export const smooth = (t, a, b) => {
  const x = Math.max(0, Math.min(1, (t - a) / (b - a)))
  return x * x * (3 - 2 * x)
}

export const gauss = (t, center, width) => Math.exp(-(((t - center) / width) ** 2))

const cross = (u, v) => [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]

/** Normal transformada pela inversa transposta do jacobiano de `warp` em `position`. */
export function warpNormal(warp, position, normal) {
  const e = 1e-4
  const origin = warp(position)
  const [a, b, c] = [0, 1, 2].map((axis) => {
    const p = [...position]
    p[axis] += e
    return warp(p).map((v, i) => (v - origin[i]) / e)
  })
  // J = [a b c]; as linhas de J⁻¹ são (b×c, c×a, a×b)/det, logo J⁻ᵀ·n combina essas linhas por n.
  const rows = [cross(b, c), cross(c, a), cross(a, b)]
  const n = [0, 1, 2].map((k) => rows[0][k] * normal[0] + rows[1][k] * normal[1] + rows[2][k] * normal[2])
  const length = Math.hypot(...n) || 1
  return n.map((v) => v / length)
}

/** Aplica `warp` a POSITION/NORMAL intercalados (Float32Array de 3 em 3), no lugar. */
export function warpInPlace(warp, positions, normals, filter = () => true) {
  for (let i = 0; i < positions.length; i += 3) {
    const point = [positions[i], positions[i + 1], positions[i + 2]]
    if (!filter(point)) continue
    normals.set(warpNormal(warp, point, [normals[i], normals[i + 1], normals[i + 2]]), i)
    positions.set(warp(point), i)
  }
}
