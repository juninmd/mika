/** Cabelo da Haru: penteado preso a partir das mechas VRoid e rabo lateral de cachos em espiral. */

import { smooth } from './haru-warp.mjs'

const STYLE = { eyes: 1.47, longest: 1.03, brow: 1.452, jaw: 1.3, nape: 1.39, center: -0.034, hugRadius: 0.118 }
/**
 * Penteado da Haru Live2D a partir das mechas longas VRoid, sem cortar triângulos (as pontas
 * afiladas continuam sendo o fim de cada mecha): franja até a sobrancelha, mechas laterais até
 * a mandíbula na frente das orelhas, e a nuca presa rente ao crânio, puxada para o lado do rabo.
 */
export function haruStylePoint([x, y, z]) {
  if (y >= STYLE.eyes) return [x, y, z]
  const depth = (STYLE.eyes - y) / (STYLE.eyes - STYLE.longest)
  const around = Math.abs(Math.atan2(x, z - STYLE.center))
  const front = 1 - smooth(around, 0.85, 1.35)
  const fringe = 1 - smooth(Math.abs(x), 0.04, 0.075)
  const end = (STYLE.brow * fringe + STYLE.jaw * (1 - fringe)) * front + STYLE.nape * (1 - front)
  const ny = STYLE.eyes - depth * (STYLE.eyes - end)
  // Mechas que caíam nos ombros encostam no crânio e no rosto; sem isso abriam em asa na mandíbula.
  const radius = Math.hypot(x, z - STYLE.center)
  const pull = smooth(depth, 0.06, 0.6)
  const hug = 1 + (Math.min(1, STYLE.hugRadius / Math.max(radius, 1e-6)) - 1) * pull
  // Presa: a nuca escorrega para o lado direito dela, onde fica o rabo.
  const gather = (1 - front) * smooth(depth, 0.3, 1) * 0.022
  const cx = x * hug - gather
  const cz = STYLE.center + (z - STYLE.center) * hug
  // Mechas laterais encostam no rosto em vez de abrir como leque.
  const lock = front * (1 - fringe) * smooth(depth, 0, 0.8)
  return [cx * (1 - lock * 0.06), ny, cz + lock * 0.004]
}
/** Cacho em espiral (drill): fita larga enrolada em hélice cônica ao redor de um eixo curvo. */
export function drillCurl(T, { top, bottom, turns = 3.6, radius = 0.03, steps = 480, around = 28, side = 1 }) {
  const axis = new T.CatmullRomCurve3([
    new T.Vector3(...top),
    new T.Vector3(top[0] + side * 0.022, top[1] - 0.06, top[2] + 0.004),
    new T.Vector3(bottom[0] - side * 0.006, bottom[1] + 0.06, bottom[2] - 0.004),
    new T.Vector3(...bottom),
  ])
  const length = axis.getLength()
  const positions = []
  const uvs = []
  const indices = []
  const frames = axis.computeFrenetFrames(steps, false)
  for (let i = 0; i <= steps; i++) {
    const t = i / steps
    const center = axis.getPointAt(t)
    const theta = t * turns * Math.PI * 2
    const grow = smooth(t, 0, 0.07)
    const tip = 1 - smooth(t, 0.9, 1)
    const r = (radius * (1 - t) ** 0.55 + 0.007) * grow
    const radial = frames.normals[i]
      .clone()
      .multiplyScalar(Math.cos(theta))
      .addScaledVector(frames.binormals[i], Math.sin(theta))
      .normalize()
    const width = (length / turns) * 0.62 * (1 - t * 0.45) * (0.35 + 0.65 * grow) * (0.25 + 0.75 * tip)
    const thick = 0.0065 * (1 - t * 0.5) * (0.3 + 0.7 * tip)
    const ribbon = center.clone().addScaledVector(radial, r)
    // Inclina a fita ao longo da hélice para que as voltas se sobreponham como mechas.
    const along = frames.tangents[i]
      .clone()
      .multiplyScalar(0.92)
      .addScaledVector(radial.clone().cross(frames.tangents[i]), 0.35)
      .normalize()
    for (let j = 0; j <= around; j++) {
      const phi = (j / around) * Math.PI * 2
      const bulge = 1 + Math.sin(phi * 2) * 0.08
      const p = ribbon
        .clone()
        .addScaledVector(along, Math.cos(phi) * width * 0.5 * bulge)
        .addScaledVector(radial, Math.sin(phi) * thick)
      positions.push(p.x, p.y, p.z)
      uvs.push(j / around, t * 2.2)
    }
  }
  for (let i = 0; i < steps; i++) {
    for (let j = 0; j < around; j++) {
      const a = i * (around + 1) + j
      const b = a + around + 1
      indices.push(a, b, a + 1, a + 1, b, b + 1)
    }
  }
  const geometry = new T.BufferGeometry()
  geometry.setAttribute('position', new T.Float32BufferAttribute(positions, 3))
  geometry.setAttribute('uv', new T.Float32BufferAttribute(uvs, 2))
  geometry.setIndex(indices)
  geometry.computeVertexNormals()
  return geometry
}
