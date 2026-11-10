import { describe, expect, it } from 'vitest'
import { layerStyle } from '../layer-styles'

describe('layer-styles — layerStyle', () => {
  it('mapeia o nome da camada pra variável CSS correspondente', () => {
    expect(layerStyle('hud')).toEqual({ zIndex: 'var(--layer-hud)' })
  })

  it('mescla estilos extras sem perder o zIndex calculado', () => {
    expect(layerStyle('avatar', { opacity: 0.5 })).toEqual({ zIndex: 'var(--layer-avatar)', opacity: 0.5 })
  })

  it('permite que o extra sobrescreva o zIndex quando explicitamente passado', () => {
    expect(layerStyle('dropdown', { zIndex: 999 })).toEqual({ zIndex: 999 })
  })
})
