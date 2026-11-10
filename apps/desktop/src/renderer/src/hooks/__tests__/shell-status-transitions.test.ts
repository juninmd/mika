import { describe, expect, it } from 'vitest'
import { nextStatusOnOfflineChange, nextStatusOnTtsStop, nextStatusOnVad } from '../shell-status-transitions'

describe('nextStatusOnTtsStop — evita travar em "speaking" quando o áudio já parou', () => {
  it('TTS ainda tocando: nunca força transição, mesmo vindo de "speaking"', () => {
    expect(nextStatusOnTtsStop(true, 'speaking')).toBeNull()
  })

  it('TTS parou e o status era "speaking": volta para idle', () => {
    expect(nextStatusOnTtsStop(false, 'speaking')).toBe('idle')
  })

  it('TTS parou mas o status era outro (ex.: "thinking"): não mexe — evita reabrir o turno errado', () => {
    expect(nextStatusOnTtsStop(false, 'thinking')).toBeNull()
  })
})

describe('nextStatusOnOfflineChange — modo offline sempre vence, saída só limpa o próprio offline', () => {
  it('entrando em offline: força "offline" não importa o status anterior', () => {
    expect(nextStatusOnOfflineChange(true, 'thinking')).toBe('offline')
  })

  it('saindo de offline com status "offline": volta para idle', () => {
    expect(nextStatusOnOfflineChange(false, 'offline')).toBe('idle')
  })

  it('saindo de offline com outro status: não sobrescreve um turno em andamento', () => {
    expect(nextStatusOnOfflineChange(false, 'thinking')).toBeNull()
  })
})

describe('nextStatusOnVad — "listening" só nasce e morre a partir de "idle"', () => {
  it('fala detectada com mic ligado e status idle: liga "listening"', () => {
    expect(nextStatusOnVad(true, true, 'idle')).toBe('listening')
  })

  it('fala detectada mas status já é "thinking": não interrompe o turno em curso', () => {
    expect(nextStatusOnVad(true, true, 'thinking')).toBeNull()
  })

  it('mic desligado durante "listening": volta para idle mesmo com VAD ainda ativo', () => {
    expect(nextStatusOnVad(true, false, 'listening')).toBe('idle')
  })

  it('fala parou durante "listening": volta para idle', () => {
    expect(nextStatusOnVad(false, true, 'listening')).toBe('idle')
  })

  it('nada relevante muda: não emite transição', () => {
    expect(nextStatusOnVad(false, false, 'idle')).toBeNull()
  })
})
