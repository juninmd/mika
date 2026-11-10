import type { VRM } from '@pixiv/three-vrm'
import { describe, expect, it } from 'vitest'
import { ARM_REST, armPoseFor } from '../vrm-body-rig'
import { writeArmPose } from '../vrm-pose'

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
