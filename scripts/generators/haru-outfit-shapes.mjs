/** Geometry for the Haru outfits: flared skirts, lace, bows, surface lines and the hair outline. */
import { garmentFront, scarfTail, skirtPoint, torsoSection } from './haru-garments.mjs'

/** Waist-to-hem skirt on Haru's pencil-skirt sections, opened into a flare with pleats or soft folds. */
function skirtSurface(T, { length, flare, pleats, depth }, angle, t, lift = 0) {
  const [x, y, z] = skirtPoint(angle, t)
  // The top hides under the torso down to its hem (y 0.855): flaring earlier would poke through it.
  const open = T.MathUtils.smoothstep(t, 0.3, length)
  // Knife pleats: a sawtooth whose steep face turns away from the light and draws the crease in the
  // cel shade. Symmetric box pleats lit both faces alike and read as a plain cylinder. The phase keeps
  // vertices off the wrap point, where float noise would flip a vertex to the other side of the step.
  const turn = ((((angle / (2 * Math.PI)) * pleats + 1 / 16) % 1) + 1) % 1
  const fold = pleats ? turn - 0.5 : Math.sin(angle * 9 + t * 3) * 0.5
  const radius = Math.hypot(x, z)
  const scale = 1 + open * flare + (fold * depth * open + lift) / radius
  return [x * scale, y, z * scale]
}

export function flaredSkirt(T, skirt) {
  const segments = skirt.pleats ? skirt.pleats * 8 : 144
  const geometry = new T.CylinderGeometry(1, 1, 1, segments, 48, true)
  const p = geometry.attributes.position
  for (let i = 0; i < p.count; i++) {
    const angle = Math.atan2(p.getZ(i), p.getX(i))
    p.setXYZ(i, ...skirtSurface(T, skirt, angle, (0.5 - p.getY(i)) * skirt.length))
  }
  geometry.computeVertexNormals()
  return geometry
}

/** Lace layer tucked under the hem that peeks out below it in scallops. */
export function petticoat(T, skirt, drop) {
  const geometry = new T.CylinderGeometry(1, 1, 1, 192, 6, true)
  const p = geometry.attributes.position
  for (let i = 0; i < p.count; i++) {
    const angle = Math.atan2(p.getZ(i), p.getX(i))
    const v = 0.5 - p.getY(i)
    const scallop = Math.abs(Math.sin(angle * 18)) * 0.35 + 0.65
    const [x, y, z] = skirtSurface(T, skirt, angle, skirt.length, -skirt.depth + v * (skirt.depth + 0.004))
    p.setXYZ(i, x, y + 0.012 - v * (0.012 + drop * scallop), z)
  }
  geometry.computeVertexNormals()
  return geometry
}

export function apron(T, skirt) {
  const geometry = new T.PlaneGeometry(1, 1, 48, 24)
  const p = geometry.attributes.position
  for (let i = 0; i < p.count; i++) {
    const u = p.getX(i) + 0.5
    const v = 0.5 - p.getY(i)
    const angle = Math.PI / 2 + (u - 0.5) * 1.5
    const end = skirt.length - 0.1 - Math.abs(Math.sin(u * Math.PI * 7)) * 0.018
    p.setXYZ(i, ...skirtSurface(T, skirt, angle, T.MathUtils.lerp(0.31, end, v), skirt.depth * 0.6 + 0.004))
  }
  geometry.computeVertexNormals()
  return geometry
}

export function waistband(T, add, name, y, mat) {
  const [width, front, back] = torsoSection(y)
  const loop = Array.from({ length: 128 }, (_, i) => {
    const angle = (i / 128) * Math.PI * 2
    const z = Math.sin(angle)
    return new T.Vector3(Math.cos(angle) * (width + 0.004), y, z * ((z > 0 ? front : back) + 0.006))
  })
  add(name, new T.TubeGeometry(new T.CatmullRomCurve3(loop, true), 256, 0.005, 8, true), mat, 'chest')
}

/** Butterfly bow: two lobes pinched toward the knot, so the outline reads as a ribbon, not two balls. */
export function bow(T, add, name, mat, bone, [x, y, z], size, tilt = 0) {
  for (const side of [-1, 1]) {
    const lobe = new T.SphereGeometry(1, 28, 14).translate(1, 0, 0)
    const p = lobe.attributes.position
    for (let i = 0; i < p.count; i++) p.setY(i, p.getY(i) * (0.3 + 0.38 * p.getX(i)))
    lobe.computeVertexNormals()
    // Mirror by rotation, not a negative scale: flipped winding turns the normals (and outline) inward.
    lobe
      .scale(size * 0.5, size * 0.5, size * 0.22)
      .rotateY(side < 0 ? Math.PI : 0)
      .rotateZ(side * 0.12 + tilt)
    add(name, lobe.translate(x, y, z), mat, bone)
  }
  add(name, new T.SphereGeometry(size * 0.2, 16, 10).scale(1, 1.1, 0.7).translate(x, y, z + size * 0.08), mat, bone)
}

export function ribbonTails(T, add, name, mat, [x, y, z], width) {
  for (const side of [-1, 1]) {
    const points = [0, 0.3, 0.65, 1].map((t) => {
      const [, , surface] = garmentFront(x + side * t * 0.03, y - t * 0.085, 0.018)
      return [x + side * (0.004 + t * 0.03), y - 0.006 - t * 0.085, Math.max(surface, z - 0.004 - t * 0.012)]
    })
    add(name, scarfTail(T, points, width), mat, 'chest')
  }
}

/** Edge a→b moved `inset` into the panel, toward its third corner `inner`. */
export function insetEdge([ax, ay], [bx, by], [cx, cy], inset) {
  const length = Math.hypot(bx - ax, by - ay)
  let nx = -(by - ay) / length
  let ny = (bx - ax) / length
  if (nx * (cx - ax) + ny * (cy - ay) < 0) [nx, ny] = [-nx, -ny]
  return [
    [ax + nx * inset, ay + ny * inset],
    [bx + nx * inset, by + ny * inset],
  ]
}

// Sampled densely on the body surface: a spline through a few projected points cuts through the chest.
export function line(T, add, name, points, surface, radius, mat, offset = 0.012) {
  const dense = points.slice(1).flatMap(([bx, by], i) => {
    const [ax, ay] = points[i]
    return Array.from({ length: 12 }, (_, k) => [ax + ((bx - ax) * k) / 12, ay + ((by - ay) * k) / 12])
  })
  dense.push(points.at(-1))
  line3(
    T,
    add,
    name,
    dense.map(([x, y]) => surface(x, y, offset)),
    radius,
    mat,
    'chest',
  )
}

export function line3(T, add, name, points, radius, mat, bone) {
  const curve = new T.CatmullRomCurve3(points.map((point) => new T.Vector3(...point)))
  add(name, new T.TubeGeometry(curve, 96, radius, 8, false), mat, bone)
}

export function crownFrill(T, head, arc, height) {
  const geometry = new T.PlaneGeometry(1, 1, arc.length - 1, 3)
  const p = geometry.attributes.position
  for (let i = 0; i < p.count; i++) {
    const u = p.getX(i) + 0.5
    const v = p.getY(i) + 0.5
    const index = Math.round(u * (arc.length - 1))
    const scallop = 0.7 + Math.abs(Math.sin(index * 1.4)) * 0.3
    p.setXYZ(i, ...head.at(arc[index], 0.002 + v * height * scallop, v * 0.006))
  }
  geometry.computeVertexNormals()
  return geometry
}

/** Hair silhouette in the crown's vertical slice: where a headband or hair bow actually sits. */
export function hairOutline(hair) {
  let top = Number.NEGATIVE_INFINITY
  let back = Number.POSITIVE_INFINITY
  let front = Number.NEGATIVE_INFINITY
  for (let i = 0; i < hair.length; i += 3) {
    top = Math.max(top, hair[i + 1])
    back = Math.min(back, hair[i + 2])
    front = Math.max(front, hair[i + 2])
  }
  const plane = back + (front - back) * 0.55
  const center = top - 0.11
  const slice = []
  for (let i = 0; i < hair.length; i += 3) {
    if (Math.abs(hair[i + 2] - plane) < 0.012 && hair[i + 1] > center - 0.06) slice.push(hair[i], hair[i + 1])
  }
  return {
    at(angle, lift, forward = 0) {
      const dx = Math.cos(angle)
      const dy = Math.sin(angle)
      let reach = 0
      for (let i = 0; i < slice.length; i += 2) {
        const x = slice[i]
        const y = slice[i + 1] - center
        if (Math.abs(x * dy - y * dx) < 0.008) reach = Math.max(reach, x * dx + y * dy)
      }
      const r = reach + lift
      return [dx * r, center + dy * r, plane + forward]
    },
  }
}
