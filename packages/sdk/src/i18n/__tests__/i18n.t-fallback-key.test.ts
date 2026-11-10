import { describe, expect, it, vi } from 'vitest'

// Forces the "key missing from the target locale's dict but present in the fallback" branch,
// which never happens in production because every catalog ships with full key parity today.
vi.mock('../translations', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../translations')>()
  const esES = await actual.loadLocale('es-ES')
  actual.translations['es-ES'] = Object.fromEntries(
    Object.entries(esES).filter(([key]) => key !== 'button.save'),
  ) as typeof esES
  return actual
})

describe('t() — fallback catalog covers a key missing from the target locale', () => {
  it('falls back to en-US for a key the target catalog dropped', async () => {
    const { t } = await import('../index')
    expect(t('button.save', 'es-ES')).toBe('Save')
  })
})
