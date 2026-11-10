/**
 * Anatomia das pernas sobre a malha VRoid (13 vértices por anel, um tubo quase reto): patela,
 * recuo poplíteo, panturrilha com pico medial mais alto que o lateral, tendão e tornozelo finos.
 * Coordenadas de bind da base, antes de proporções e subdivisão; medidas em `mika.vrm`.
 */
import { gauss, smooth } from './haru-warp.mjs'

const LEG = { x: 0.077, hip: 0.84, knee: 0.505, ankle: 0.18, sole: 0.11 }

// Centro do anel em z segue a linha coxa→joelho→tornozelo do esqueleto e o volume da panturrilha.
const centerZ = (y) =>
  y > LEG.knee ? -0.007 * smooth(y, LEG.hip, LEG.knee) : -0.007 - 0.03 * smooth(y, LEG.knee, 0.12)

export function isLeg([x, y]) {
  return Math.abs(x) > 0.004 && y > LEG.sole && y < LEG.hip + 0.02
}

export function sculptLeg([x, y, z]) {
  const side = Math.sign(x)
  const lx = x - side * LEG.x
  const lz = z - centerZ(y)
  const outer = lx * side > 0
  const fade = 1 - smooth(y, 0.78, LEG.hip + 0.02)
  const foot = smooth(y, LEG.sole, 0.14)
  const knee = gauss(y, LEG.knee, 0.035)
  const ankle = gauss(y, LEG.ankle, 0.055) * foot
  const calf = outer ? gauss(y, 0.36, 0.06) : gauss(y, 0.39, 0.05)
  const innerThigh = outer ? 0 : gauss(y, 0.66, 0.08)
  const width = 1 + fade * (-0.07 * knee + (outer ? 0.05 : 0.07) * calf - 0.15 * ankle - 0.06 * innerThigh)
  const depth =
    lz < 0
      ? 1 + fade * (0.16 * calf - 0.08 * knee - 0.22 * ankle)
      : 1 + fade * (-0.03 * calf + 0.02 * knee - 0.12 * ankle)
  const patella = lz > 0 ? 0.005 * knee * gauss(lx, 0, 0.022) * fade : 0
  return [side * LEG.x + lx * width, y, centerZ(y) + lz * depth + patella]
}
