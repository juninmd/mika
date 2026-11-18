import type { VRM } from '@pixiv/three-vrm'
import { describe, expect, it } from 'vitest'
import { ARM_REST, armPoseFor } from '../vrm-body-rig'
import { animateVrmHead, SpeechEnvelope, writeArmPose } from '../vrm-pose'

function fakeVrm() {
  type Node = { rotation: { values: number[]; set: (...values: number[]) => void } }
  const nodes = new Map<string, Node>()
  const get = (name: string) => {
    const node: Node = { rotation: { values: [], set: (...values) => Object.assign(node.rotation, { values }) } }
    nodes.set(name, node)
    return node
  }
  return {
    humanoid: { getNormalizedBoneNode: get },
    nodes,
  } as unknown as VRM & { nodes: typeof nodes }
}

describe('writeArmPose', () => {
  it('escreve paz bilateral com indicadores e médios estendidos e polegares fechados', () => {
    const vrm = fakeVrm()
    writeArmPose(vrm, { ...ARM_REST, ...armPoseFor('22', 0.5) })

    for (const side of ['left', 'right'] as const) {
      const sign = side === 'left' ? -1 : 1
      expect(vrm.nodes.get(`${side}IndexProximal`)?.rotation.values[2]).toBeCloseTo(0)
      expect(vrm.nodes.get(`${side}MiddleProximal`)?.rotation.values[2]).toBeCloseTo(0)
      expect(vrm.nodes.get(`${side}RingProximal`)?.rotation.values[2]).toBeCloseTo(sign * 0.99)
      expect(vrm.nodes.get(`${side}LittleProximal`)?.rotation.values[2]).toBeCloseTo(sign * 0.99)
      expect(vrm.nodes.get(`${side}ThumbMetacarpal`)?.rotation.values).toEqual([0, -sign * 0.35, sign * 0.5])
      expect(vrm.nodes.get(`${side}ThumbProximal`)?.rotation.values).toEqual([0, -sign * 0.3, sign * 0.55])
      expect(vrm.nodes.get(`${side}ThumbDistal`)?.rotation.values).toEqual([0, 0, sign * 0.45])
    }
  })
})

describe('suavidade da cabeça e da fala', () => {
  const FRAME = 1 / 60

  // O giro do peso era zerado no quadro em que o aceno começava: a cabeça estalava de lado.
  it('começar um aceno não faz a cabeça estalar de lado', () => {
    const head = { rotation: { x: 0, y: 0, z: 0 } } as Parameters<typeof animateVrmHead>[0]
    animateVrmHead(head, 0, 0, 1, 0, 0, 'none')
    const before = head.rotation.y
    animateVrmHead(head, 1, 0, 1, 0, 0, 'none')
    expect(Math.abs(head.rotation.y - before)).toBeLessThan(0.005)
  })

  // O volume cru do analisador oscila a cada quadro; multiplicado direto, o gesto da fala tremia.
  it('volume ruidoso não faz a intensidade do gesto tremer', () => {
    const envelope = new SpeechEnvelope()
    for (let i = 0; i < 60; i++) envelope.advance(true, 0.5, FRAME)
    let prev = envelope.advance(true, 0.5, FRAME)
    let worst = 0
    for (let i = 0; i < 120; i++) {
      const next = envelope.advance(true, i % 2 ? 1 : 0, FRAME)
      worst = Math.max(worst, Math.abs(next - prev))
      prev = next
    }
    expect(worst).toBeLessThan(0.03)
  })

  // Parar de falar sumia com o gesto num quadro: tronco e braços pulavam de volta ao repouso.
  it('parar de falar solta o gesto aos poucos até zerar', () => {
    const envelope = new SpeechEnvelope()
    for (let i = 0; i < 120; i++) envelope.advance(true, 1, FRAME)
    const speaking = envelope.advance(true, 1, FRAME)
    const first = envelope.advance(false, 0, FRAME)
    expect(speaking - first).toBeLessThan(0.02)
    let last = first
    for (let i = 0; i < 90; i++) last = envelope.advance(false, 0, FRAME)
    expect(last).toBeLessThan(0.05)
  })
})
