import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { type GripFrame, gripFrameInto, gripQuaternionInto, solveTwoBoneInto } from '../vrm-grip'

const v = (x: number, y: number, z: number) => new THREE.Vector3(x, y, z)
const frame = (): GripFrame => ({ center: v(0, 0, 0), axis: v(0, 0, 0), normal: v(0, 0, 0) })

/** Mão direita em T-pose (modelo olhando +Z): dedos para -X, polegar para +Z, palma para baixo. */
const RIGHT = {
  wrist: v(-0.6, 1.4, 0),
  index: v(-0.68, 1.4, 0.025),
  middle: v(-0.69, 1.4, 0),
  little: v(-0.67, 1.4, -0.04),
}

describe('quadro de empunhadura a partir dos ossos da mão', () => {
  it('põe o cabo sob a palma, atravessando-a do mindinho para o indicador', () => {
    const f = gripFrameInto(frame(), RIGHT.wrist, RIGHT.index, RIGHT.middle, RIGHT.little, -1)
    expect(f.normal.y).toBeLessThan(-0.99)
    // Eixo sai do lado do polegar (+Z) e inclina para os dedos (-X), como a diagonal de uma pegada real.
    expect(f.axis.z).toBeGreaterThan(0.8)
    expect(f.axis.x).toBeLessThan(-0.1)
    expect(Math.abs(f.axis.dot(f.normal))).toBeLessThan(1e-6)
    // Centro entre o punho e os nós dos dedos, abaixo da palma — nunca no osso do punho.
    expect(f.center.x).toBeLessThan(RIGHT.wrist.x - 0.03)
    expect(f.center.x).toBeGreaterThan(RIGHT.middle.x)
    expect(f.center.y).toBeLessThan(RIGHT.wrist.y - 0.01)
  })

  it('a mão esquerda espelha a direita: palma para baixo, cabo saindo pelo polegar', () => {
    const mirror = (p: THREE.Vector3) => v(-p.x, p.y, p.z)
    const f = gripFrameInto(
      frame(),
      mirror(RIGHT.wrist),
      mirror(RIGHT.index),
      mirror(RIGHT.middle),
      mirror(RIGHT.little),
      1,
    )
    expect(f.normal.y).toBeLessThan(-0.99)
    expect(f.axis.z).toBeGreaterThan(0.8)
    expect(f.axis.x).toBeGreaterThan(0.1)
  })

  it('acompanha qualquer orientação da mão: girar os ossos gira o quadro igual', () => {
    const q = new THREE.Quaternion().setFromEuler(new THREE.Euler(0.7, -1.2, 2.1))
    const turn = (p: THREE.Vector3) => p.clone().applyQuaternion(q)
    const base = gripFrameInto(frame(), RIGHT.wrist, RIGHT.index, RIGHT.middle, RIGHT.little, -1)
    const turned = gripFrameInto(
      frame(),
      turn(RIGHT.wrist),
      turn(RIGHT.index),
      turn(RIGHT.middle),
      turn(RIGHT.little),
      -1,
    )
    expect(turned.axis.distanceTo(base.axis.clone().applyQuaternion(q))).toBeLessThan(1e-6)
    expect(turned.normal.distanceTo(base.normal.clone().applyQuaternion(q))).toBeLessThan(1e-6)
    expect(turned.center.distanceTo(turn(base.center))).toBeLessThan(1e-6)
  })

  it('a rotação da espada leva +Y ao eixo do cabo e o fio (+X) para o lado dos dedos', () => {
    const f = gripFrameInto(frame(), RIGHT.wrist, RIGHT.index, RIGHT.middle, RIGHT.little, -1)
    const q = gripQuaternionInto(new THREE.Quaternion(), f.axis, f.normal)
    expect(v(0, 1, 0).applyQuaternion(q).distanceTo(f.axis)).toBeLessThan(1e-6)
    expect(v(1, 0, 0).applyQuaternion(q).distanceTo(f.normal)).toBeLessThan(1e-6)
  })
})

describe('IK analítico de dois ossos', () => {
  const root = v(0.2, 1.4, 0)
  const mid = v(0.45, 1.4, 0)
  const end = v(0.7, 1.4, 0)
  const lengths = (m: THREE.Vector3, e: THREE.Vector3) => [m.distanceTo(root), e.distanceTo(m)]

  it('alcança o alvo preservando os comprimentos do braço, com o cotovelo do lado do polo', () => {
    const target = v(0.1, 1.3, 0.3)
    const outMid = v(0, 0, 0)
    const outEnd = v(0, 0, 0)
    solveTwoBoneInto(outMid, outEnd, root, mid, end, target, v(0.4, 1.0, 0))
    expect(outEnd.distanceTo(target)).toBeLessThan(1e-6)
    const [a, b] = lengths(outMid, outEnd)
    expect(a).toBeCloseTo(0.25, 6)
    expect(b).toBeCloseTo(0.25, 6)
    expect(outMid.y).toBeLessThan(root.y)
  })

  it('fora de alcance estica em direção ao alvo sem esticar os ossos', () => {
    const target = v(1.5, 1.4, 0.5)
    const outMid = v(0, 0, 0)
    const outEnd = v(0, 0, 0)
    solveTwoBoneInto(outMid, outEnd, root, mid, end, target, v(0.4, 1.0, 0))
    const [a, b] = lengths(outMid, outEnd)
    expect(a).toBeCloseTo(0.25, 6)
    expect(b).toBeCloseTo(0.25, 6)
    const toward = target.clone().sub(root).normalize()
    expect(outEnd.clone().sub(root).normalize().dot(toward)).toBeGreaterThan(0.999)
  })

  it('polo degenerado (sobre a linha até o alvo) ainda dá um cotovelo finito', () => {
    const target = v(0.5, 1.4, 0)
    const outMid = v(0, 0, 0)
    const outEnd = v(0, 0, 0)
    solveTwoBoneInto(outMid, outEnd, root, v(0.45, 1.4, 0), end, target, v(0.9, 1.4, 0))
    expect(Number.isFinite(outMid.x + outMid.y + outMid.z)).toBe(true)
    expect(outEnd.distanceTo(target)).toBeLessThan(1e-6)
  })
})
