import { describe, expect, it } from 'vitest'
import { isForegroundToolEvent } from '../tool-call-origin'

describe('isForegroundToolEvent', () => {
  it('turno de chat visível acende o HUD', () => {
    expect(isForegroundToolEvent('turn')).toBe(true)
  })

  it('job de fundo (fato/contexto/humor pós-turno) não acende o HUD', () => {
    expect(isForegroundToolEvent('background')).toBe(false)
  })

  it('evento sem origin (plugin/código antigo) é tratado como turno — nunca esconde de mais', () => {
    expect(isForegroundToolEvent(undefined)).toBe(true)
  })
})
