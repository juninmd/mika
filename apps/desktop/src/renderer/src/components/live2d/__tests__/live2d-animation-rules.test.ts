import { describe, expect, it } from 'vitest'
import { LIVE2D_ACTION_CATALOG } from '../live2d-action-catalog.generated'
import { EXPRESSION_ACTION_MAP } from '../live2d-animator-config'
import { getMoodForExpression, getReactionActionNames, shouldLeanForwardForExpression } from '../live2d-expression'
import { getLipSyncMultiplier, getLipSyncTarget } from '../live2d-lip-sync'

describe('Live2D expression rules', () => {
  it.each([
    ['thinking', 'focused', true],
    ['happy', 'energetic', false],
    ['bored', 'tired', false],
    ['surprised', 'neutral', false],
  ])('maps %s to its established mood and lean state', (expression, mood, leansForward) => {
    expect(getMoodForExpression(expression)).toBe(mood)
    expect(shouldLeanForwardForExpression(expression)).toBe(leansForward)
  })

  it('cai no neutro — sem ação — para expressão desconhecida', () => {
    expect(getReactionActionNames('unknown-expression')).toEqual([])
  })

  it('mapeia toda expressão conhecida para ações que existem no catálogo gerado', () => {
    const known = new Set(Object.values(LIVE2D_ACTION_CATALOG).flatMap((skin) => Object.keys(skin)))

    for (const [expression, actions] of Object.entries(EXPRESSION_ACTION_MAP)) {
      for (const action of actions) {
        expect(known, `expressão '${expression}' aponta para ação inexistente '${action}'`).toContain(action)
      }
    }
  })

  it('mantém a skin padrão com todas as ações', () => {
    expect(Object.keys(LIVE2D_ACTION_CATALOG.haru_greeter_t05 ?? {}).sort()).toEqual([
      'angry',
      'cheer',
      'happy',
      'laugh',
      'nod',
      'pout',
      'sad',
      'shakehead',
      'shrug',
      'shy',
      'sleepy',
      'surprised',
      'thinking',
      'wave',
      'wink',
    ])
  })
})

describe('Live2D lip-sync rules', () => {
  it.each([
    ['excited', 1.25],
    ['happy', 1.15],
    ['angry', 1.1],
    ['sad', 0.8],
    ['thinking', 0.85],
    ['neutral', 1],
  ])('uses the established %s multiplier', (expression, multiplier) => {
    expect(getLipSyncMultiplier(expression)).toBe(multiplier)
  })

  it.each(['excited', 'happy', 'angry', 'sad', 'thinking', 'neutral', 'love', 'panic'])(
    'mantém %s num ganho que não satura a fala normal',
    (expression) => {
      expect(getLipSyncMultiplier(expression)).toBeLessThanOrEqual(1.5)
    },
  )

  it('uses manual mouth openness without an audio volume', () => {
    expect(getLipSyncTarget(0.42, undefined, 'happy')).toBe(0.42)
  })

  it('scales audio volume by expression and clamps mouth openness', () => {
    expect(getLipSyncTarget(0, 0.2, 'happy')).toBeCloseTo(0.23)

    expect(getLipSyncTarget(0, 0.5, 'excited')).toBeCloseTo(0.625)

    expect(getLipSyncTarget(0, 1, 'excited')).toBe(1)
  })
})
