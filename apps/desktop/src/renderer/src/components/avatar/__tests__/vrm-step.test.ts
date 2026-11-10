import type { VRM } from '@pixiv/three-vrm'
import { describe, expect, it } from 'vitest'
import { stepVrm } from '../vrm-scene-setup'

function fakeVrm() {
  const springs: number[] = []
  const once: string[] = []
  const vrm = {
    humanoid: { update: () => once.push('humanoid') },
    lookAt: { update: (dt: number) => once.push(`lookAt:${dt}`) },
    expressionManager: { update: () => once.push('expressions') },
    springBoneManager: { update: (dt: number) => springs.push(dt) },
    materials: [{ update: (dt: number) => once.push(`material:${dt}`) }],
  }
  return { vrm: vrm as unknown as VRM, springs, once }
}

// Pedido do usuário (2026-09-28): gestos rápidos com movimento secundário sem tremer nem explodir —
// a 20 fps um passo de mola de 50 ms fazia o cabelo saltar.
describe('passo do VRM', () => {
  it('a fps baixo a mola anda em subpassos curtos que somam o quadro inteiro', () => {
    const { vrm, springs, once } = fakeVrm()
    stepVrm(vrm, 0.05)
    expect(springs.length).toBeGreaterThan(1)
    for (const step of springs) expect(step).toBeLessThanOrEqual(1 / 30 + 1e-9)
    expect(springs.reduce((a, b) => a + b, 0)).toBeCloseTo(0.05, 9)
    // Pose, olhar, expressões e materiais andam uma vez com o quadro inteiro.
    expect(once).toEqual(['humanoid', 'lookAt:0.05', 'expressions', 'material:0.05'])
  })

  it('a 60 fps é um passo só', () => {
    const { vrm, springs } = fakeVrm()
    stepVrm(vrm, 1 / 60)
    expect(springs).toEqual([1 / 60])
  })
})
