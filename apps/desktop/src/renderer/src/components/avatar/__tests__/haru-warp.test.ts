import { describe, expect, it } from 'vitest'
import { warpNormal } from '../../../../../../../../scripts/generators/haru-warp.mjs'

// Cisalhamento x' = x + y/2: resposta analítica conhecida para as normais.
const shear = ([x, y, z]: number[]) => [x + 0.5 * y, y, z]
const close = (actual: number[], expected: number[]) => {
  const length = Math.hypot(...expected)
  expected.forEach((value, i) => {
    expect(actual[i]).toBeCloseTo(value / length, 4)
  })
}

describe('normal de deformação em espaço de bind', () => {
  it('plano que o cisalhamento leva nele mesmo conserva a normal (J⁻¹ a inclinaria e o cel quebraria)', () => {
    close(warpNormal(shear, [0.2, 0, 0.1], [0, 1, 0]), [0, 1, 0])
  })

  it('plano x = 0 vira x = y/2, com normal (1, -1/2, 0)', () => {
    close(warpNormal(shear, [0, 0.3, 0], [1, 0, 0]), [1, -0.5, 0])
  })
})
