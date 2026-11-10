import { describe, expect, it } from 'vitest'
import { isGlowMaterial, planFor, TARGET_BUDGET_BYTES, targetBytes } from '../vrm-postfx'

describe('plano de qualidade do render do VRM (MSAA + supersampling)', () => {
  it('tela comum (DPR 1) e janela pequena: supersampling 2x com MSAA 4x', () => {
    expect(planFor('full', 400, 700, 1, 8)).toEqual({ supersample: 2, samples: 4 })
  })

  it('tela densa (DPR >= 2) já tem pixels de sobra: sem supersampling, MSAA no máximo', () => {
    expect(planFor('full', 600, 700, 2, 8)).toEqual({ supersample: 1, samples: 8 })
  })

  it('janela grande em tela densa: o MSAA cede antes de estourar a memória', () => {
    expect(planFor('full', 1280, 800, 2, 8)).toEqual({ supersample: 1, samples: 4 })
  })

  it('janela cheia em 1080p estoura o orçamento de memória: cai de MSAA antes de largar o supersampling', () => {
    const plan = planFor('full', 1920, 1080, 1, 8)
    expect(plan.supersample).toBe(2)
    expect(plan.samples).toBeLessThan(4)
    expect(targetBytes(1920, 1080, plan)).toBeLessThanOrEqual(TARGET_BUDGET_BYTES)
  })

  it('nunca passa do orçamento, em qualquer tamanho', () => {
    for (const [w, h, dpr] of [
      [640, 800, 1],
      [2560, 1440, 1],
      [3840, 2160, 1],
      [3840, 2160, 2],
      [5120, 2880, 2],
    ] as const) {
      const plan = planFor('full', w, h, dpr, 8)
      expect(targetBytes(w, h, plan), `${w}x${h}@${dpr}`).toBeLessThanOrEqual(TARGET_BUDGET_BYTES)
    }
  })

  it('bateria / segundo plano: sem supersampling e MSAA menor', () => {
    expect(planFor('reduced', 400, 700, 1, 8)).toEqual({ supersample: 1, samples: 4 })
    expect(planFor('eco', 400, 700, 1, 8)).toEqual({ supersample: 1, samples: 2 })
  })

  it('respeita o MSAA que a GPU oferece', () => {
    expect(planFor('full', 600, 700, 2, 4).samples).toBe(4)
    expect(planFor('full', 600, 700, 2, 0)).toEqual({ supersample: 1, samples: 0 })
  })

  it('pausado cai no plano mais barato', () => {
    expect(planFor('paused', 400, 700, 1, 8).supersample).toBe(1)
  })
})

describe('bloom só nas áreas emissivas', () => {
  it('olhos e detalhes luminosos brilham; pele, cabelo e roupa lisa não', () => {
    for (const name of [
      'EyeIris_00_EYE',
      'EyeHighlight_00_EYE',
      'Renewal_Cuff_Rib',
      'Renewal_arcade_Hem',
      'Renewal_1_SockStripe',
      'Renewal_Gamepad_Material',
      'Renewal_PixelHeart_Material',
      'Aurora_SignalMint',
    ]) {
      expect(isGlowMaterial(name), name).toBe(true)
    }
    for (const name of [
      'Body_00_SKIN',
      'Face_00_SKIN',
      'Hair_00_HAIR',
      'EyeWhite_00_EYE',
      'Renewal_Sleeves',
      'Renewal_Jacket',
      'Renewal_arcade_Skirt',
    ]) {
      expect(isGlowMaterial(name), name).toBe(false)
    }
  })

  it('o nome do contorno herda o do material', () => {
    expect(isGlowMaterial('EyeIris_00_EYE (Outline)')).toBe(true)
    expect(isGlowMaterial('Hair_00_HAIR (Outline)')).toBe(false)
  })
})
