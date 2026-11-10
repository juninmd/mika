import { describe, expect, it } from 'vitest'
import { createMicStateGuard } from '../mic-state-guard'

/** A guarda separa eco do próprio `sendMicState` de mudança externa (`voice:listen_start` do LLM), e corta rajada. */
describe('mic-state-guard', () => {
  it('ignora o eco do mesmo valor que o renderer acabou de enviar', () => {
    const guard = createMicStateGuard({ limit: 5, windowMs: 1000 })

    expect(guard.shouldSend(true, 0)).toBe(true)
    expect(guard.shouldApplyEcho(true, 5)).toBe(false)
    expect(guard.shouldApplyEcho(true, 20)).toBe(false)
  })

  it('aplica mudança iniciada fora do renderer (valor diferente do enviado)', () => {
    const guard = createMicStateGuard({ limit: 5, windowMs: 1000 })

    expect(guard.shouldApplyEcho(true, 0)).toBe(true)

    expect(guard.shouldSend(false, 10)).toBe(true)
    expect(guard.shouldApplyEcho(true, 20)).toBe(true)
  })

  it('bloqueia rajada acima do limite dentro da janela', () => {
    const guard = createMicStateGuard({ limit: 3, windowMs: 1000 })

    expect(guard.shouldSend(true, 0)).toBe(true)
    expect(guard.shouldSend(false, 100)).toBe(true)
    expect(guard.shouldSend(true, 200)).toBe(true)
    expect(guard.shouldSend(false, 300)).toBe(false)

    // Envio bloqueado não vira "último enviado": o eco dele é mudança para a guarda.
    expect(guard.shouldApplyEcho(false, 301)).toBe(true)
    expect(guard.shouldApplyEcho(true, 301)).toBe(false)
  })

  it('a janela deslizante libera depois de windowMs', () => {
    const guard = createMicStateGuard({ limit: 2, windowMs: 1000 })

    expect(guard.shouldSend(true, 0)).toBe(true)
    expect(guard.shouldSend(false, 10)).toBe(true)
    expect(guard.shouldSend(true, 20)).toBe(false)

    expect(guard.shouldSend(true, 1001)).toBe(true)
  })

  // Smoke 2026-09-27: botão OFF com a aba STT dizendo "Escutando agora". Envio cortado pela rajada
  // era descartado para sempre — UI e backend ficavam em estados opostos até o próximo clique.
  it('diz quando o envio cortado pode ser refeito, para o último valor nunca se perder', () => {
    const guard = createMicStateGuard({ limit: 3, windowMs: 1000 })

    expect(guard.retryAfterMs(0)).toBe(0)
    guard.shouldSend(true, 0)
    guard.shouldSend(false, 100)
    guard.shouldSend(true, 200)
    expect(guard.shouldSend(false, 300)).toBe(false)

    const wait = guard.retryAfterMs(300)
    expect(wait).toBe(700)
    expect(guard.shouldSend(false, 300 + wait)).toBe(true)
    expect(guard.shouldApplyEcho(false, 300 + wait)).toBe(false)
  })

  it('reset limpa o último envio e a janela', () => {
    const guard = createMicStateGuard({ limit: 1, windowMs: 1000 })

    expect(guard.shouldSend(true, 0)).toBe(true)
    expect(guard.shouldSend(false, 10)).toBe(false)

    guard.reset()

    expect(guard.shouldSend(false, 20)).toBe(true)
    expect(guard.shouldApplyEcho(true, 20)).toBe(true)
  })
})
