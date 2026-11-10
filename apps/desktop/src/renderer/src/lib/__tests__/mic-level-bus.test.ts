import { beforeEach, describe, expect, it, vi } from 'vitest'
import { getMicLevel, publishMicLevel, resetMicLevel, subscribeMicLevel } from '../mic-level-bus'

/** O HUD só prova que o microfone chega na Mika se o nível sair do laço de VAD. */
describe('mic-level-bus', () => {
  beforeEach(() => {
    resetMicLevel()
  })

  it('entrega cada amostra a todos os assinantes', () => {
    const a = vi.fn()
    const b = vi.fn()
    subscribeMicLevel(a)
    const stopB = subscribeMicLevel(b)

    publishMicLevel({ level: 0.42, hearing: true })

    expect(a).toHaveBeenCalledWith({ level: 0.42, hearing: true })
    expect(b).toHaveBeenCalledWith({ level: 0.42, hearing: true })
    stopB()
  })

  it('para de entregar depois do unsubscribe — a onda não sobrevive à desmontagem', () => {
    const listener = vi.fn()
    subscribeMicLevel(listener)()

    publishMicLevel({ level: 0.9, hearing: true })

    expect(listener).not.toHaveBeenCalled()
  })

  it('quem assina depois lê o último nível, sem esperar o próximo tick', () => {
    publishMicLevel({ level: 0.31, hearing: true })
    expect(getMicLevel()).toEqual({ level: 0.31, hearing: true })
  })

  it('reset zera: microfone pausado não pode deixar a onda congelada no último pico', () => {
    publishMicLevel({ level: 0.8, hearing: true })
    resetMicLevel()
    expect(getMicLevel()).toEqual({ level: 0, hearing: false })
  })
})
