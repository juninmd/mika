import { describe, expect, it } from 'vitest'
import {
  IDLE_MOTION_MAX_INTERVAL_SEC,
  IDLE_MOTION_MIN_INTERVAL_SEC,
  randomIdleAction,
  VrmIdleScheduler,
} from '../vrm-idle'

const NOON = () => 12

function fixedRand(...values: number[]): () => number {
  let i = 0
  return () => values[Math.min(i++, values.length - 1)]
}

describe('randomIdleAction', () => {
  it('só escolhe poses sutis: uma ação reativa (wave, cheer, heart...) disparada sem gatilho pareceria uma resposta a nada', () => {
    for (let rand = 0; rand < 1; rand += 0.05) {
      const action = randomIdleAction(rand)
      expect(['stretch', 'sigh', 'curious', 'relaxed', 'thinking', 'shrug', 'smug', 'yawn', 'drink']).toContain(action)
    }
  })

  it('não estoura o índice na borda superior (rand = 1)', () => {
    expect(() => randomIdleAction(1)).not.toThrow()
  })

  it('não estoura o índice com rand negativo', () => {
    expect(() => randomIdleAction(-1)).not.toThrow()
  })

  it('tolera rand inválido e pode excluir a pose anterior', () => {
    expect(randomIdleAction(Number.NaN)).toBe('stretch')
    expect(randomIdleAction(0, 'stretch')).not.toBe('stretch')
  })
})

describe('VrmIdleScheduler', () => {
  it('não dispara antes do intervalo sorteado vencer', () => {
    const scheduler = new VrmIdleScheduler(fixedRand(0.5), NOON)
    const result = scheduler.tick(IDLE_MOTION_MIN_INTERVAL_SEC - 0.01, { busy: false, enabled: true })
    expect(result).toBeNull()
  })

  it('dispara assim que o intervalo vence e sorteia um novo intervalo dentro dos limites', () => {
    const scheduler = new VrmIdleScheduler(fixedRand(0.5, 0.9), NOON)
    const first = scheduler.tick(IDLE_MOTION_MAX_INTERVAL_SEC, { busy: false, enabled: true })
    expect(first).not.toBeNull()

    // O timer zerou; sem mais tempo acumulado, um novo tick não deveria repetir na mesma janela.
    const immediate = scheduler.tick(0, { busy: false, enabled: true })
    expect(immediate).toBeNull()
  })

  it('reinicia o relógio enquanto ocupado para não disparar logo depois de uma ação ou fala', () => {
    const scheduler = new VrmIdleScheduler(fixedRand(0.5), NOON)
    scheduler.tick(IDLE_MOTION_MIN_INTERVAL_SEC, { busy: false, enabled: true })
    scheduler.tick(1, { busy: true, enabled: true })
    const result = scheduler.tick(IDLE_MOTION_MIN_INTERVAL_SEC, { busy: false, enabled: true })
    expect(result).toBeNull()
  })

  it('reinicia enquanto desabilitado (janela em segundo plano ou prefers-reduced-motion)', () => {
    const scheduler = new VrmIdleScheduler(fixedRand(0.5), NOON)
    scheduler.tick(IDLE_MOTION_MIN_INTERVAL_SEC, { busy: false, enabled: true })
    scheduler.tick(1, { busy: false, enabled: false })
    const result = scheduler.tick(IDLE_MOTION_MIN_INTERVAL_SEC, { busy: false, enabled: true })
    expect(result).toBeNull()
  })

  it('ignora dt inválido (NaN, Infinity, negativo) em vez de corromper o relógio', () => {
    const scheduler = new VrmIdleScheduler(fixedRand(0.5), NOON)
    expect(() => scheduler.tick(Number.NaN, { busy: false, enabled: true })).not.toThrow()
    expect(() => scheduler.tick(Number.POSITIVE_INFINITY, { busy: false, enabled: true })).not.toThrow()
    expect(scheduler.tick(-5, { busy: false, enabled: true })).toBeNull()
  })

  it('retoma a contagem do zero depois de um gate, não do ponto onde parou', () => {
    const scheduler = new VrmIdleScheduler(fixedRand(0.5), NOON)
    scheduler.tick(IDLE_MOTION_MIN_INTERVAL_SEC, { busy: true, enabled: true })
    const stillGated = scheduler.tick(0.5, { busy: false, enabled: true })
    expect(stillGated).toBeNull()
  })

  it('não repete a mesma pose em dois ciclos consecutivos', () => {
    const scheduler = new VrmIdleScheduler(fixedRand(0), NOON)
    const first = scheduler.tick(IDLE_MOTION_MAX_INTERVAL_SEC, { busy: false, enabled: true })
    const second = scheduler.tick(IDLE_MOTION_MAX_INTERVAL_SEC, { busy: false, enabled: true })

    expect(first).toBe('stretch')
    expect(second).not.toBe(first)
  })
})

// "Ela precisa ter reações como sentir sono": late at night the idle body shows it.
describe('sono de madrugada', () => {
  const picks = (hour: number) => {
    const scheduler = new VrmIdleScheduler(Math.random, () => hour)
    const seen: string[] = []
    for (let i = 0; i < 400; i++) {
      const action = scheduler.tick(IDLE_MOTION_MAX_INTERVAL_SEC, { busy: false, enabled: true })
      if (action) seen.push(action)
    }
    return seen.filter((a) => a === 'yawn' || a === 'stretch' || a === 'doze').length / seen.length
  }

  it('boceja, cochila e se espreguiça bem mais entre 23h e 5h do que à tarde', () => {
    expect(picks(1)).toBeGreaterThan(0.45)
    expect(picks(15)).toBeLessThan(0.35)
  })
})

describe('cochilo na madrugada', () => {
  it('entre as poses de sono de madrugada também aparece o cochilo', () => {
    const scheduler = new VrmIdleScheduler(Math.random, () => 2)
    const seen = new Set<string>()
    for (let i = 0; i < 200; i++) {
      const action = scheduler.tick(IDLE_MOTION_MAX_INTERVAL_SEC, { busy: false, enabled: true })
      if (action) seen.add(action)
    }
    expect(seen).toContain('doze')
  })
})
