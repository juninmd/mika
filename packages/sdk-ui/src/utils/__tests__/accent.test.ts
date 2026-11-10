import { describe, expect, it } from 'vitest'
import { accentVars } from '../accent'

/** Guards: absent hue paints nothing (AccentContext relies on this to inherit from parent); defined hue always writes all 8 variables. */

describe('accentVars', () => {
  it('hue undefined devolve undefined — nenhum estilo é aplicado', () => {
    expect(accentVars(undefined)).toBeUndefined()
  })

  it('hue 0 é um valor válido, não deve ser tratado como ausente', () => {
    const vars = accentVars(0)
    expect(vars).toBeDefined()
    expect(vars?.['--accent-h' as never]).toBe('0')
  })

  // Same lightness as the root token: at 62% `text-accent` fell to 4.25:1 on raised surfaces.
  it('sem options, usa saturation=90% e lightness=65%, o mesmo da raiz', () => {
    const vars = accentVars(200)
    expect(vars?.['--accent-s' as never]).toBe('90%')
    expect(vars?.['--accent-l' as never]).toBe('65%')
    expect(vars?.['--accent' as never]).toBe('hsl(200 90% 65%)')
  })

  it('options.saturation/lightness sobrescrevem os defaults em todas as variáveis derivadas', () => {
    const vars = accentVars(120, { saturation: 50, lightness: 40 })
    expect(vars?.['--accent-s' as never]).toBe('50%')
    expect(vars?.['--accent' as never]).toBe('hsl(120 50% 40%)')
    expect(vars?.['--accent-soft' as never]).toBe('hsl(120 50% 40% / 0.14)')
  })

  it('--accent-text usa sempre lightness fixo 78%, independente da lightness base', () => {
    const vars = accentVars(300, { lightness: 20 })
    expect(vars?.['--accent-text' as never]).toBe('hsl(300 90% 78%)')
  })

  it('as 8 variáveis de acento estão todas presentes quando hue é definido', () => {
    const vars = accentVars(10) ?? {}
    expect(Object.keys(vars).sort()).toEqual(
      [
        '--accent',
        '--accent-glow',
        '--accent-h',
        '--accent-l',
        '--accent-rim',
        '--accent-s',
        '--accent-soft',
        '--accent-text',
      ].sort(),
    )
  })
})
