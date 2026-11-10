/** Seções de alfaiataria: largura, profundidade frontal e dorsal variam independentemente. */
const TORSO_SECTIONS = [
  [0.855, 0.153, 0.103, 0.128],
  [0.89, 0.142, 0.1, 0.111],
  [0.945, 0.116, 0.078, 0.09],
  [1, 0.096, 0.071, 0.067],
  [1.055, 0.092, 0.072, 0.058],
  [1.115, 0.102, 0.095, 0.058],
  [1.18, 0.123, 0.138, 0.067],
  [1.23, 0.127, 0.126, 0.069],
  [1.275, 0.12, 0.085, 0.063],
  [1.305, 0.091, 0.063, 0.057],
  [1.345, 0.052, 0.045, 0.045],
]
const SKIRT_SECTIONS = [
  [0.53, 0.145, 0.096, 0.111],
  [0.61, 0.139, 0.092, 0.108],
  [0.685, 0.137, 0.093, 0.108],
  [0.75, 0.139, 0.096, 0.116],
  [0.815, 0.145, 0.1, 0.125],
  [0.855, 0.148, 0.098, 0.122],
  [0.89, 0.134, 0.09, 0.102],
  [0.945, 0.105, 0.066, 0.078],
  [1.005, 0.081, 0.056, 0.055],
]

export function torsoSection(y) {
  return sectionAt(TORSO_SECTIONS, y)
}

function sectionAt(sections, y) {
  const index = sections.findIndex((section) => section[0] >= y)
  if (index === 0) return sections[0].slice(1)
  if (index === -1) return sections.at(-1).slice(1)
  const a = sections[index - 1],
    b = sections[index]
  const t = (y - a[0]) / (b[0] - a[0])
  const span = b[0] - a[0]
  // Hermite monotônico: tangentes contínuas, sem degraus entre cintura e quadril.
  return a.slice(1).map((value, i) => {
    const start = sectionSlope(sections, index - 1, i + 1) * span
    const end = sectionSlope(sections, index, i + 1) * span
    return (
      (2 * t ** 3 - 3 * t ** 2 + 1) * value +
      (t ** 3 - 2 * t ** 2 + t) * start +
      (-2 * t ** 3 + 3 * t ** 2) * b[i + 1] +
      (t ** 3 - t ** 2) * end
    )
  })
}

function sectionSlope(sections, index, axis) {
  const current = sections[index]
  const previous = sections[Math.max(0, index - 1)]
  const next = sections[Math.min(sections.length - 1, index + 1)]
  const before = current[0] - previous[0]
  const after = next[0] - current[0]
  if (before === 0) return (next[axis] - current[axis]) / after
  if (after === 0) return (current[axis] - previous[axis]) / before
  const left = (current[axis] - previous[axis]) / before
  const right = (next[axis] - current[axis]) / after
  if (left * right <= 0) return 0
  const w1 = 2 * after + before,
    w2 = after + 2 * before
  return (w1 + w2) / (w1 / left + w2 / right)
}

export function garmentFront(x, y, offset = 0.004) {
  const [width, depth] = torsoSection(y)
  return [x, y, depth * Math.sqrt(Math.max(0, 1 - (x / width) ** 2)) + frontTailoring(x, y) + offset]
}

export function garmentBack(x, y, offset = 0.004) {
  const [width, , back] = torsoSection(y)
  return [x, y, -back * Math.sqrt(Math.max(0, 1 - (x / width) ** 2)) - offset]
}

function frontTailoring(x, y) {
  const bust = Math.exp(-(((y - 1.183) / 0.047) ** 2))
  const separation = -0.007 * Math.exp(-((x / 0.026) ** 2)) * bust
  const waist = Math.exp(-(((y - 1.068) / 0.052) ** 2))
  const folds = Math.sin((y - Math.abs(x) * 0.32) * 155) * 0.0018 * waist
  return separation + folds
}

/** Subdivide as lapelas para acompanharem o volume do tronco sem atravessar o blazer. */
export function curvedPanel(T, points, offset = 0.009, surface = garmentFront) {
  const vertices = []
  const divisions = 16
  const triangles = T.ShapeUtils.triangulateShape(
    points.map(([x, y]) => new T.Vector2(x, y)),
    [],
  )
  for (const indices of triangles) {
    // Seen from behind the winding reverses; without the swap the back panel shows only its outline hull.
    const ordered = surface === garmentFront ? indices : [indices[0], indices[2], indices[1]]
    const [a, b, c] = ordered.map((index) => points[index])
    const point = (u, v) =>
      surface(
        a[0] + ((b[0] - a[0]) * u) / divisions + ((c[0] - a[0]) * v) / divisions,
        a[1] + ((b[1] - a[1]) * u) / divisions + ((c[1] - a[1]) * v) / divisions,
        offset,
      )
    for (let u = 0; u < divisions; u++) {
      for (let v = 0; v < divisions - u; v++) {
        vertices.push(...point(u, v), ...point(u + 1, v), ...point(u, v + 1))
        if (u + v < divisions - 1) vertices.push(...point(u + 1, v), ...point(u + 1, v + 1), ...point(u, v + 1))
      }
    }
  }
  const geometry = new T.BufferGeometry()
  geometry.setAttribute('position', new T.Float32BufferAttribute(vertices, 3))
  const normals = []
  for (let i = 0; i < vertices.length; i += 3) {
    const x = vertices[i],
      y = vertices[i + 1]
    const origin = new T.Vector3(...surface(x, y, offset))
    const dx = new T.Vector3(...surface(x + 0.0001, y, offset)).sub(origin)
    const dy = new T.Vector3(...surface(x, y + 0.0001, offset)).sub(origin)
    // The back surface faces -Z: flip so its normals still point out of the body.
    const normal = dx.cross(dy).normalize()
    normals.push(...(surface === garmentFront ? normal : normal.negate()).toArray())
  }
  geometry.setAttribute('normal', new T.Float32BufferAttribute(normals, 3))
  return geometry
}

export function tailoredTorso(T) {
  const geometry = new T.CylinderGeometry(1, 1, 0.49, 128, 96, true).translate(0, 1.1, 0)
  const p = geometry.attributes.position
  for (let i = 0; i < p.count; i++) {
    const y = p.getY(i),
      x = p.getX(i),
      z = p.getZ(i)
    const [width, front, back] = torsoSection(y)
    const seam = Math.sin(Math.atan2(z, x) * 6) * 0.0008
    p.setXYZ(i, x * width, y, z * (z > 0 ? front : back) + seam + (z > 0 ? frontTailoring(x * width, y) * z : 0))
  }
  geometry.computeVertexNormals()
  return geometry
}

export function skirtPoint(angle, t) {
  const y = 1.005 - t * 0.475
  const [width, front, back] = sectionAt(SKIRT_SECTIONS, y)
  const z = Math.sin(angle)
  const drape = Math.sin(angle * 6 + t * 7) * 0.0016 * Math.sin(t * Math.PI)
  return [Math.cos(angle) * (width + drape), y, z * ((z > 0 ? front : back) + drape)]
}

export function fittedSkirt(T) {
  const geometry = new T.CylinderGeometry(1, 1, 1, 128, 64, true)
  const p = geometry.attributes.position
  for (let i = 0; i < p.count; i++) {
    p.setXYZ(i, ...skirtPoint(Math.atan2(p.getZ(i), p.getX(i)), 0.5 - p.getY(i)))
  }
  geometry.computeVertexNormals()
  return geometry
}

export function scarfWrap(T) {
  const geometry = new T.CylinderGeometry(1, 1, 1, 96, 24, true)
  const p = geometry.attributes.position
  const uv = geometry.attributes.uv
  for (let i = 0; i < p.count; i++) {
    const x = p.getX(i),
      z = p.getZ(i),
      t = p.getY(i)
    const angle = Math.atan2(z, x)
    const fold = Math.cos(angle * 3 + t * 2) * 0.0018
    p.setXYZ(i, x * (0.057 + fold), 1.337 + t * 0.028 - Math.max(z, 0) * 0.008, z * (0.052 + fold))
    uv.setXY(i, uv.getX(i) * 2, uv.getY(i) * 0.4)
  }
  geometry.computeVertexNormals()
  return geometry
}

export function scarfTail(T, points, width) {
  const curve = new T.CatmullRomCurve3(points.map((point) => new T.Vector3(...point)))
  const geometry = new T.PlaneGeometry(1, 1, 16, 64)
  const positions = geometry.attributes.position
  for (let i = 0; i < positions.count; i++) {
    const t = 0.5 - positions.getY(i),
      across = positions.getX(i) * 2
    const tangent = curve.getTangentAt(t).normalize()
    const lateral = new T.Vector3(-tangent.y, tangent.x, 0).normalize()
    const taper = 0.7 + Math.sin(t * Math.PI) * 0.3 - T.MathUtils.smoothstep(t, 0.8, 1) * 0.2
    const point = curve.getPointAt(t).addScaledVector(lateral, across * width * taper)
    point.z += Math.sin(across * Math.PI + t * 5) * 0.004 * Math.sin(t * Math.PI)
    point.y += across * across * T.MathUtils.smoothstep(t, 0.88, 1) * 0.008
    positions.setXYZ(i, ...point.toArray())
  }
  geometry.computeVertexNormals()
  return geometry
}

/** Manga contínua encolhendo suavemente do ombro até o punho. */
export function tailoredSleeve(T, shoulder, elbow, wrist) {
  const upper = elbow.clone().sub(shoulder)
  const lower = wrist.clone().sub(elbow)
  const split = upper.length() / (upper.length() + lower.length())
  const start = shoulder.clone().addScaledVector(upper.clone().normalize(), -0.045)
  const curve = new T.CatmullRomCurve3([
    start,
    shoulder.clone().lerp(elbow, 0.5),
    elbow,
    elbow.clone().lerp(wrist, 0.5),
    wrist,
  ])
  const geometry = new T.TubeGeometry(curve, 64, 1, 32, false)
  const positions = geometry.attributes.position
  for (let i = 0; i < positions.count; i++) {
    const t = Math.floor(i / 33) / 64
    const angle = ((i % 33) / 32) * Math.PI * 2
    const center = curve.getPointAt(t)
    const radius = T.MathUtils.lerp(0.04, 0.027, t) * (0.2 + 0.8 * T.MathUtils.smoothstep(t, 0, 0.07))
    const elbowFold = Math.exp(-(((t - split) * 14) ** 2)) * Math.sin(t * 85 + Math.cos(angle) * 1.4) * 0.0013
    const point = new T.Vector3().fromBufferAttribute(positions, i)
    point
      .sub(center)
      .multiplyScalar(radius + elbowFold)
      .add(center)
    positions.setXYZ(i, ...point.toArray())
  }
  geometry.computeVertexNormals()
  const axis = wrist.clone().sub(shoulder).normalize()
  const elbowDistance = elbow.clone().sub(shoulder).dot(axis)
  return {
    geometry,
    lowerWeight(positions, i) {
      const distance = new T.Vector3().fromBufferAttribute(positions, i).sub(shoulder).dot(axis)
      return T.MathUtils.smoothstep(distance, elbowDistance - 0.055, elbowDistance + 0.055)
    },
  }
}
