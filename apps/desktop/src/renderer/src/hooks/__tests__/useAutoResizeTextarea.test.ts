import { describe, expect, it } from 'vitest'
import { computeResizedHeight } from '../useAutoResizeTextarea'

describe('useAutoResizeTextarea — computeResizedHeight', () => {
  it('usa a altura do conteúdo quando ela cabe dentro do teto', () => {
    expect(computeResizedHeight(40, 120)).toBe(40)
  })

  it('trava no teto quando o conteúdo é maior que o máximo permitido', () => {
    expect(computeResizedHeight(500, 120)).toBe(120)
  })

  it('é idempotente no limite exato (borda)', () => {
    expect(computeResizedHeight(120, 120)).toBe(120)
  })

  it('trata altura zero (textarea vazia) sem virar negativa', () => {
    expect(computeResizedHeight(0, 120)).toBe(0)
  })
})
