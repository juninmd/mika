import { describe, expect, it } from 'vitest'
import { nextRelationshipIdForKey, type Relationship } from '../RelationshipSelector'

const OPTIONS: Array<{ id: Relationship }> = [
  { id: 'acquaintance' },
  { id: 'friend' },
  { id: 'close-friend' },
  { id: 'companion' },
]

describe('RelationshipSelector — nextRelationshipIdForKey', () => {
  it('avança para o próximo item com ArrowRight', () => {
    expect(nextRelationshipIdForKey('ArrowRight', OPTIONS, 'acquaintance')).toBe('friend')
  })

  it('avança para o próximo item com ArrowDown (equivalente a ArrowRight)', () => {
    expect(nextRelationshipIdForKey('ArrowDown', OPTIONS, 'friend')).toBe('close-friend')
  })

  it('volta para o item anterior com ArrowLeft', () => {
    expect(nextRelationshipIdForKey('ArrowLeft', OPTIONS, 'friend')).toBe('acquaintance')
  })

  it('dá a volta (wrap-around) do último para o primeiro item', () => {
    expect(nextRelationshipIdForKey('ArrowRight', OPTIONS, 'companion')).toBe('acquaintance')
  })

  it('dá a volta (wrap-around) do primeiro para o último item', () => {
    expect(nextRelationshipIdForKey('ArrowLeft', OPTIONS, 'acquaintance')).toBe('companion')
  })

  it('ignora teclas que não são de navegação', () => {
    expect(nextRelationshipIdForKey('Enter', OPTIONS, 'friend')).toBeNull()
  })

  it('retorna null quando o id atual não existe na lista (estado inconsistente)', () => {
    expect(nextRelationshipIdForKey('ArrowRight', OPTIONS, 'ghost' as Relationship)).toBeNull()
  })

  it('retorna null com lista vazia em vez de estourar índice', () => {
    expect(nextRelationshipIdForKey('ArrowRight', [], 'friend')).toBeNull()
  })
})
