import { describe, expect, it } from 'vitest'
import { loadFeatureUi } from '../feature-ui-registry'

describe('feature-ui-registry', () => {
  it('retorna null para feature inexistente', () => {
    expect(loadFeatureUi('non_existent_feature')).toBeNull()
  })

  it('carrega e faz cache da promise de UI para features conhecidas', async () => {
    const p1 = loadFeatureUi('mika.mood')
    expect(p1).toBeInstanceOf(Promise)
    const p2 = loadFeatureUi('mika.mood')
    expect(p1).toBe(p2) // Same cached promise
    const ui = await p1
    expect(ui).toBeDefined()
  })
})
