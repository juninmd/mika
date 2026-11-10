import { beforeEach, describe, expect, it } from 'vitest'
import { createScene } from '../../components/effects/scenes'
import { useEffectsStore } from '../../store/useEffectsStore'
import { type MatchEffectCommand, parseMatchEffectToolResult } from '../match-effect'

describe('parseMatchEffectToolResult', () => {
  const commandOf = (result: unknown): MatchEffectCommand | null => parseMatchEffectToolResult(result)

  it('envelope de comando direto vira toggle', () => {
    expect(commandOf({ success: true, data: { type: 'toggle', effect: 'matrix', triggered: true } })).toEqual({
      effect: 'matrix',
      mode: 'toggle',
    })
  })

  it('envelope do sorteio ("surpreenda-me") vira show', () => {
    expect(commandOf({ success: true, data: { type: 'random', effect: 'sakura', triggered: true } })).toEqual({
      effect: 'sakura',
      mode: 'show',
    })
  })

  it('efeito que a tool não ativou não vira comando', () => {
    expect(commandOf({ success: true, data: { triggered: false } })).toBeNull()
  })

  it('falha da tool não vira comando', () => {
    expect(commandOf({ success: false, error: 'x' })).toBeNull()
  })

  it('efeito desconhecido não vira comando — não há cena desenhada para ele', () => {
    expect(commandOf({ success: true, data: { type: 'toggle', effect: 'holodeck', triggered: true } })).toBeNull()
  })

  it('envelope malformado não derruba o chamador', () => {
    expect(commandOf(null)).toBeNull()
    expect(commandOf('ok')).toBeNull()
    expect(commandOf({ success: true })).toBeNull()
    expect(commandOf({ success: true, data: 'matrix' })).toBeNull()
  })

  it('tipo futuro sem cena conhecida acende por show — acende e o usuário desliga', () => {
    expect(commandOf({ success: true, data: { effect: 'matrix', triggered: true } })).toEqual({
      effect: 'matrix',
      mode: 'show',
    })
  })
})

describe('useEffectsStore', () => {
  beforeEach(() => {
    useEffectsStore.setState({ activeEffect: null, startedAt: 0 })
  })

  it('toggle acende o efeito', () => {
    useEffectsStore.getState().applyMatchEffect({ effect: 'matrix', mode: 'toggle' })
    expect(useEffectsStore.getState().activeEffect).toBe('matrix')
  })

  it('toggle do MESMO efeito apaga — é assim que o usuário desliga o /matrix', () => {
    useEffectsStore.getState().applyMatchEffect({ effect: 'matrix', mode: 'toggle' })
    useEffectsStore.getState().applyMatchEffect({ effect: 'matrix', mode: 'toggle' })
    expect(useEffectsStore.getState().activeEffect).toBeNull()
  })

  it('toggle de outro efeito substitui, não apaga', () => {
    useEffectsStore.getState().applyMatchEffect({ effect: 'matrix', mode: 'toggle' })
    useEffectsStore.getState().applyMatchEffect({ effect: 'sakura', mode: 'toggle' })
    expect(useEffectsStore.getState().activeEffect).toBe('sakura')
  })

  it('show acende mesmo repetido — sorteio re-sortear não é "desligar"', () => {
    useEffectsStore.getState().applyMatchEffect({ effect: 'fireworks', mode: 'show' })
    useEffectsStore.getState().applyMatchEffect({ effect: 'fireworks', mode: 'show' })
    expect(useEffectsStore.getState().activeEffect).toBe('fireworks')
  })
})

function fakeCtx() {
  const calls = { fillText: 0, arc: 0, stroke: 0, fill: 0, ellipse: 0, clearRect: 0, gradients: 0 }
  const ctx = {
    clearRect: () => calls.clearRect++,
    fillText: () => calls.fillText++,
    beginPath: () => {},
    arc: () => calls.arc++,
    ellipse: () => calls.ellipse++,
    fill: () => calls.fill++,
    stroke: () => calls.stroke++,
    moveTo: () => {},
    lineTo: () => {},
    save: () => {},
    restore: () => {},
    translate: () => {},
    rotate: () => {},
    createLinearGradient: () => {
      calls.gradients++
      return { addColorStop: () => {} }
    },
    set fillStyle(_v: string) {},
    set strokeStyle(_v: string) {},
    set font(_v: string) {},
    set textAlign(_v: string) {},
    set lineWidth(_v: number) {},
  }
  return { ctx: ctx as unknown as CanvasRenderingContext2D, calls }
}

describe('cenas', () => {
  const TIERS = ['full', 'reduced', 'eco'] as const
  const EFFECTS = ['matrix', 'constellation', 'sakura', 'synthwave', 'fireworks'] as const

  it.each(EFFECTS)('%s desenha sem lançar em qualquer faixa de qualidade', (effect) => {
    for (const tier of TIERS) {
      const { ctx, calls } = fakeCtx()
      const scene = createScene(effect, tier)
      for (let i = 0; i < 120; i++) scene.step(ctx, 16, 1280, 720)

      expect(calls.clearRect).toBeGreaterThan(0)
    }
  })

  it('matrix desenha glifos de texto', () => {
    const { ctx, calls } = fakeCtx()
    const scene = createScene('matrix', 'full')
    scene.step(ctx, 16, 1280, 720)
    scene.step(ctx, 16, 1280, 720)
    expect(calls.fillText).toBeGreaterThan(0)
  })

  it('fireworks se encerra sozinho — comemoração não vira papel de parede', () => {
    const { ctx } = fakeCtx()
    const scene = createScene('fireworks', 'full')
    for (let i = 0; i < 800 && !scene.done; i++) scene.step(ctx, 16, 1280, 720)
    expect(scene.done).toBe(true)
  })

  it('cenas ambientais não se encerram sozinhas — toggle é quem desliga', () => {
    for (const effect of ['matrix', 'constellation', 'sakura', 'synthwave'] as const) {
      const { ctx } = fakeCtx()
      const scene = createScene(effect, 'full')
      for (let i = 0; i < 120; i++) scene.step(ctx, 16, 1280, 720)
      expect(scene.done).toBe(false)
    }
  })

  it('faixa de qualidade reduz a população, não a apaga', () => {
    const full = fakeCtx()
    const eco = fakeCtx()
    const sFull = createScene('constellation', 'full')
    const sEco = createScene('constellation', 'eco')
    for (let i = 0; i < 10; i++) {
      sFull.step(full.ctx, 16, 1280, 720)
      sEco.step(eco.ctx, 16, 1280, 720)
    }
    expect(eco.calls.arc).toBeGreaterThan(0)
    expect(eco.calls.arc).toBeLessThan(full.calls.arc)
  })
})
